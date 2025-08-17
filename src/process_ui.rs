use crate::AppDataHandle;
use crate::AppEvent;
use crate::il::{Disassembly, ExecutionState, MemMap, Message, StackFrame};
use proc_maps::get_process_maps;
use read_process_memory::CopyAddress;
use std::collections::HashMap;
use std::path::{self, PathBuf};

#[derive(Debug)]
pub struct ProcessState {
    pub frames: Option<Vec<StackFrame>>,
    pub gdb_execution_state: ExecutionState,
    pub registers: HashMap<String, u64>,
    pub memory_maps: Option<Vec<MemMap>>,
    pub disassembly: HashMap<u64, Disassembly>,
    pub cs_disassembly: HashMap<u64, Disassembly>,
    pub events_history: Vec<AppEvent>,
    pub memory_probes: HashMap<String, (u64, Vec<u8>)>,
    pub child_pid: Option<u64>,
    pub environment_cwd: Option<PathBuf>,
    pub elfs: HashMap<String, crate::elf::Elf>,
}

impl ProcessState {
    pub fn new() -> Self {
        Self {
            frames: None,
            gdb_execution_state: ExecutionState::default(),
            registers: HashMap::new(),
            memory_maps: None,
            disassembly: HashMap::new(),
            cs_disassembly: HashMap::new(),
            events_history: Vec::new(),
            memory_probes: HashMap::new(),
            child_pid: None,
            environment_cwd: None,
            elfs: HashMap::new(),
        }
    }

    pub fn tick(&mut self, app: &AppDataHandle) {
        self.lazy_update_cs_disassembly();
    }

    pub fn update(&mut self, event: &AppEvent, app: &AppDataHandle) {
        self.update_memory_maps(event);
        self.update_registers(event);
        self.update_callstack(event);
        self.update_disassembly(event);
        self.update_pid(event);
        self.update_cwd(event);
        self.events_history.push(event.clone());

        self.ask_update_mem(event, app);
    }

    pub fn update_pid(&mut self, event: &AppEvent) {
        match event {
            AppEvent::IL(Message::Pid(pid)) => {
                self.child_pid = Some(*pid);
            }
            _ => {}
        }
    }

    pub fn telescope(&self, addr: u64, mut acc: Vec<u64>) -> Option<Vec<u64>> {
        match self.addr_memory_map(addr) {
            _ if acc.len() > 8 => None,
            None if acc.is_empty() => None,
            Some(map) if map.map_range.is_read() => {
                let bytes = self.read_memory_bytes(addr, 8).unwrap_or(vec![]);

                let len = 8.min(bytes.len());
                let mut buf = [0u8; 8];
                buf[..len].copy_from_slice(&bytes[..len]);

                let next_addr = u64::from_le_bytes(buf);
                acc.push(addr);
                self.telescope(next_addr, acc)
            }

            _ => {
                let v = addr;
                acc.push(v);
                Some(acc)
            }
        }
    }

    pub fn read_memory_bytes(&self, addr: u64, size: u64) -> Option<Vec<u8>> {
        match self.addr_memory_map(addr) {
            None => None,
            Some(_) => {
                let h: read_process_memory::ProcessHandle =
                    (self.child_pid.unwrap() as i32).try_into().unwrap();
                read_process_memory::copy_address(addr as usize, size as usize, &h).ok()
            }
        }
    }

    pub fn ask_update_mem(&self, event: &AppEvent, app: &AppDataHandle) {
        match event {
            AppEvent::IL(_) => {
                for (reg_name, maybe_addr) in self.registers.iter() {
                    if let Some(_map) = self.addr_memory_map(*maybe_addr) {
                        app.try_read_mem(*maybe_addr, 8);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn addr_memory_map(&self, addr: u64) -> Option<MemMap> {
        if self.memory_maps.is_none() {
            None
        } else {
            self.memory_maps
                .as_ref()
                .unwrap()
                .iter()
                .find(|map| {
                    let value = addr as usize;
                    value >= map.map_range.start()
                        && (value < map.map_range.start() + map.map_range.size())
                })
                .cloned()
        }
    }

    pub fn addr_memory_perm(&self, addr: u64) -> Option<(bool, bool, bool)> {
        let range = self.addr_memory_map(addr);
        if range.is_some() {
            let r = range.as_ref().unwrap().map_range.is_read();
            let w = range.as_ref().unwrap().map_range.is_write();
            let x = range.as_ref().unwrap().map_range.is_exec();
            Some((r, w, x))
        } else {
            None
        }
    }

    fn lazy_update_cs_disassembly(&mut self) {
        let rip = self.registers.get("rip");
        if self.cs_disassembly.is_empty() {
            self.capstone_update_read_all_execmem();
        } else if rip.is_some() && self.cs_disassembly.get(rip.unwrap()).is_none() {
            //println!("lazy update!!");
            //self.force_update_cs_disassembly();
            crate::capstone_disassembly::read_map_containing_address(
                *rip.unwrap(),
                self.memory_maps.as_ref().unwrap(),
                self.child_pid.unwrap(),
                &mut self.cs_disassembly,
            );

            crate::capstone_disassembly::read_at(
                *rip.unwrap(),
                self.memory_maps.as_ref().unwrap(),
                self.child_pid.unwrap(),
                &mut self.cs_disassembly,
            );
        }
    }

    fn capstone_update_read_all_execmem(&mut self) {
        if let (Some(pid), Some(maps)) = (self.child_pid, &self.memory_maps) {
            self.cs_disassembly
                .extend((crate::capstone_disassembly::get_all_disassembly(maps, pid)));
        }
    }

    fn update_disassembly(&mut self, event: &AppEvent) {
        match event {
            AppEvent::IL(Message::Disassembly(asm_lines)) => {
                for asm in asm_lines.iter() {
                    self.disassembly.insert(asm.addr as u64, asm.clone());
                }
            }
            _ => {}
        }
    }

    fn update_registers(&mut self, event: &AppEvent) {
        match event {
            AppEvent::IL(Message::RegisterValue(regsv)) => {
                for (k, v) in regsv.iter() {
                    self.registers.insert(k.clone(), *v);
                    let rmem = self.read_memory_bytes(*v, 128);
                    match rmem {
                        Some(mem) => self.memory_probes.insert(k.clone(), (*v, mem)),
                        None => self.memory_probes.remove(k),
                    };
                }
            }
            _ => {}
        }
    }

    fn update_memory_maps(&mut self, event: &AppEvent) {
        match event {
            AppEvent::IL(Message::Maps(mps)) => {
                mps.iter()
                    .filter(|mp| mp.map_range.filename().is_some())
                    .for_each(|m| {
                        let f = m.map_range.filename().unwrap();
                        let path_str = f.as_os_str().to_str().unwrap();
                        if !self.elfs.contains_key(path_str) {
                            match crate::elf::Elf::new(path_str) {
                                Ok(elf) => {
                                    self.elfs.insert(path_str.to_string(), elf);
                                }
                                Err(e) => {
                                    //println!("{} {:?}", path_str, e);
                                }
                            }
                        }
                    });

                self.memory_maps = Some(mps.clone());
            }

            _ => {}
        }
    }

    fn update_callstack(&mut self, event: &AppEvent) {
        match event {
            AppEvent::IL(Message::StackFrames(frames)) => {
                self.frames = Some(frames.clone());
                self.frames.as_mut().unwrap().sort_by_key(|f| f.depth);
            }
            _ => {}
        }
    }

    fn update_cwd(&mut self, event: &AppEvent) {
        match event {
            AppEvent::IL(Message::Cwd(cwd)) => {
                self.environment_cwd = PathBuf::try_from(cwd).ok();
            }
            _ => {}
        }
    }
}

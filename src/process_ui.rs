use crate::AppDataHandle;
use crate::{AppEvent, Disassembly, GdbMessage, GdbState, MemMap, StackFrame};
use proc_maps::get_process_maps;
use read_process_memory::CopyAddress;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ProcessState {
    pub frames: Option<Vec<StackFrame>>,
    pub gdb_execution_state: GdbState,
    pub registers: HashMap<String, u64>,
    pub memory_maps: Option<Vec<MemMap>>,
    pub disassembly: HashMap<u64, Disassembly>,
    pub cs_disassembly: Option<HashMap<u64, Disassembly>>,
    pub events_history: Vec<AppEvent>,
    pub memory_probes: HashMap<String, (u64, Vec<u8>)>,
    pub child_pid: Option<u64>,
    pub environment_cwd: Option<PathBuf>,
}

impl ProcessState {
    pub fn new() -> Self {
        Self {
            frames: None,
            gdb_execution_state: GdbState::default(),
            registers: HashMap::new(),
            memory_maps: None,
            disassembly: HashMap::new(),
            cs_disassembly: None,
            events_history: Vec::new(),
            memory_probes: HashMap::new(),
            child_pid: None,
            environment_cwd: None,
        }
    }

    pub fn update(&mut self, event: &AppEvent, app: &AppDataHandle) {
        self.update_memory_maps(event);
        self.update_registers(event);
        self.update_callstack(event);
        self.update_disassembly(event);
        self.lazy_update_cs_disassembly();
        self.update_pid(event);
        self.update_cwd(event);
        self.events_history.push(event.clone());

        self.ask_update_mem(event, app);
    }

    pub fn update_pid(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Gdb(GdbMessage::Pid(pid)) => {
                self.child_pid = Some(*pid);
            }
            _ => {}
        }
    }

    pub fn telescope(&self, addr: u64, mut acc: Vec<u64>) -> Option<Vec<u64>> {
        match self.addr_memory_map(addr) {
            _ if acc.len() > 8 => None,
            None if acc.is_empty() => None,
            None => Some(acc),
            Some(_) => {
                let bytes = self.read_memory_bytes(addr, 8).unwrap_or(vec![]);

                let len = 8.min(bytes.len());
                let mut buf = [0u8; 8];
                buf[..len].copy_from_slice(&bytes[..len]);

                let nadr = u64::from_le_bytes(buf);
                let mut ans = vec![nadr];
                acc.append(&mut ans);
                match self.telescope(nadr, acc) {
                    None => {}
                    Some(mut seq) => ans.append(&mut seq),
                };
                Some(ans)
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
            AppEvent::Gdb(_) => {
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
        if self.cs_disassembly.is_none() {
            self.force_update_cs_disassembly();
        } else if rip.is_some()
            && self
                .cs_disassembly
                .as_ref()
                .unwrap()
                .get(rip.unwrap())
                .is_none()
        {
            self.force_update_cs_disassembly();
        }
    }

    fn force_update_cs_disassembly(&mut self) {
        if let (Some(pid), Some(maps)) = (self.child_pid, &self.memory_maps) {
            self.cs_disassembly = Some(crate::capstone_disassembly::get_all_disassembly(maps, pid));
        }
    }

    fn update_disassembly(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Gdb(GdbMessage::DisassemblyNative(asm_lines)) => {
                for asm in asm_lines.iter() {
                    self.disassembly.insert(asm.addr as u64, asm.clone());
                }
            }
            _ => {}
        }
    }

    fn update_registers(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Gdb(GdbMessage::RegisterValue(regsv)) => {
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
            AppEvent::Gdb(GdbMessage::Maps(mps)) => {
                self.memory_maps = Some(mps.clone());
            }

            _ => {}
        }
    }

    fn update_callstack(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Gdb(GdbMessage::StackFrames(frames)) => {
                self.frames = Some(frames.clone());
                self.frames.as_mut().unwrap().sort_by_key(|f| f.depth);
            }
            _ => {}
        }
    }

    fn update_cwd(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Gdb(GdbMessage::Cwd(cwd)) => {
                self.environment_cwd = PathBuf::try_from(cwd).ok();
            }
            _ => {}
        }
    }
}

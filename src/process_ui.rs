use crate::AppDataHandle;
use crate::{AppEvent, Disassembly, GdbMessage, GdbState, MemMap, StackFrame};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ProcessState {
    pub frames: Option<Vec<StackFrame>>,
    pub gdb_execution_state: GdbState,
    pub registers: HashMap<String, u64>,
    pub memory_maps: Option<Vec<MemMap>>,
    pub disassembly: HashMap<u64, Disassembly>,
    pub events_history: Vec<AppEvent>,
}

impl ProcessState {
    pub fn new() -> Self {
        Self {
            frames: None,
            gdb_execution_state: GdbState::default(),
            registers: HashMap::new(),
            memory_maps: None,
            disassembly: HashMap::new(),
            events_history: Vec::new(),
        }
    }

    pub fn update(&mut self, event: &AppEvent, app: &AppDataHandle) {
        self.update_memory_maps(event);
        self.update_registers(event);
        self.update_callstack(event);
        self.update_disassembly(event);
        self.events_history.push(event.clone());

        self.update_mem(event, app);
    }

    pub fn update_mem(&self, event: &AppEvent, app: &AppDataHandle) {
        match event {
            AppEvent::Gdb(_) => {
                for maybe_addr in self.registers.values() {
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
}

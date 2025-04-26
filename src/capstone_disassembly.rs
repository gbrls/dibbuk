use crate::{Disassembly, MemMap};
use capstone::prelude::*;
use std::collections::HashMap;

pub fn get_all_disassembly(memory_maps: &[MemMap], pid: u64) -> HashMap<u64, Disassembly> {
    let cs = Capstone::new()
        .x86()
        .mode(arch::x86::ArchMode::Mode64)
        .detail(true)
        .syntax(arch::x86::ArchSyntax::Intel)
        .build()
        .unwrap();

    let mut instructions = HashMap::new();
    for MemMap { map_range: map } in memory_maps {
        let h: read_process_memory::ProcessHandle = (pid as i32).try_into().unwrap();
        if map.is_exec() {
            //eprintln!("exec start? {:#05x}", map.start());
            if let Ok(mem) = read_process_memory::copy_address(map.start(), map.size(), &h) {
                if let Ok(cs) = cs.disasm_all(mem.as_slice(), map.start() as u64) {
                    for ins in cs.iter() {
                        instructions.insert(
                            ins.address(),
                            Disassembly {
                                str: format!("{}", ins.op_str().unwrap()),
                                operand: Some(format!("{}", ins.op_str().unwrap())),
                                mnemonic: Some(format!("{}", ins.mnemonic().unwrap())),
                                func: String::new(),
                                offset: ins.address() as usize,
                                addr: ins.address() as usize,
                            },
                        );
                    }
                }
            }
        }
    }
    instructions
}

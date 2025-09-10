use crate::debugger::runtime::MapRange;
use crate::il::{self, Disassembly};
use capstone::prelude::*;
use read_process_memory::copy_address;
use std::collections::HashMap;

pub fn capstone2dbk(ins: &capstone::Insn) -> Disassembly {
    Disassembly {
        source: il::DisassemblyEngine::CS,
        str: format!("{}", ins.op_str().unwrap()),
        operand: Some(format!("{}", ins.op_str().unwrap())),
        mnemonic: Some(format!("{}", ins.mnemonic().unwrap())),
        func: String::new(),
        offset: ins.address() as usize,
        addr: ins.address() as usize,
    }
}

// BUG: The address returned here is wrong sometimes, specially on linked libraries
pub fn disasm_in_map(map: MapRange, pid: u64, offset: usize) -> HashMap<u64, Disassembly> {
    let h: read_process_memory::ProcessHandle = (pid as i32).try_into().unwrap();
    let cs = Capstone::new()
        .x86()
        .mode(arch::x86::ArchMode::Mode64)
        .detail(true)
        .syntax(arch::x86::ArchSyntax::Intel)
        .build()
        .unwrap();

    let mut instructions = HashMap::new();
    if map.is_exec() {
        if let Ok(mem) = copy_address(map.start() + offset, map.size(), &h)
            && let Ok(cs) = cs.disasm_all(mem.as_slice(), (map.start() + offset) as u64)
        {
            for ins in cs.iter() {
                instructions.insert(ins.address(), capstone2dbk(ins));
            }
        }
    }

    instructions
}

pub fn get_all_disassembly(memory_maps: &[MapRange], pid: u64) -> HashMap<u64, Disassembly> {
    let cs = Capstone::new()
        .x86()
        .mode(arch::x86::ArchMode::Mode64)
        .detail(true)
        .syntax(arch::x86::ArchSyntax::Intel)
        .build()
        .unwrap();

    let mut instructions = HashMap::new();
    for map in memory_maps {
        instructions.extend(disasm_in_map(map.clone(), pid, 0).into_iter());
    }
    instructions
}

pub fn read_map_containing_address(
    addr: u64,
    memory_maps: &[MapRange],
    pid: u64,
    disasm: &mut HashMap<u64, Disassembly>,
) {
    let map = memory_maps.iter().find(|map| {
        //let addr = addr as usize - map.map_range.offset;
        let addr = addr as usize;
        map.start() <= addr && (map.start() + map.size()) > addr
    });

    if map.is_none() {
        return;
    }

    let map = map.unwrap();

    //println!("off {:#018x}\n", addr - map.map_range.start() as u64);

    disasm.extend(disasm_in_map(map.clone(), pid, 0).into_iter());
}

pub fn read_at(addr: u64, map: MapRange, pid: u64) -> HashMap<u64, Disassembly> {
    let offset = addr as usize - map.start();
    // let offset = addr as usize - map.start() + map.offset;

    disasm_in_map(map.clone(), pid, offset as usize)
}

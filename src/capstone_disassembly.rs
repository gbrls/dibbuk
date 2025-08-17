use crate::il::{self, Disassembly, MemMap};
use capstone::prelude::*;
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

pub fn disasm_in_map(map: &MemMap, pid: u64, offset: usize) -> HashMap<u64, Disassembly> {
    let h: read_process_memory::ProcessHandle = (pid as i32).try_into().unwrap();
    let cs = Capstone::new()
        .x86()
        .mode(arch::x86::ArchMode::Mode64)
        .detail(true)
        .syntax(arch::x86::ArchSyntax::Intel)
        .build()
        .unwrap();

    let MemMap { map_range: map } = map;
    let mut instructions = HashMap::new();
    if map.is_exec() {
        match read_process_memory::copy_address(map.start() + offset, map.size(), &h) {
            // maybe
            // offset is incorrect
            Ok(mem) => match cs.disasm_all(mem.as_slice(), (map.start() + offset) as u64) {
                Ok(cs) => {
                    //println!("csok! {:?}", cs);
                    for ins in cs.iter() {
                        instructions.insert(ins.address(), capstone2dbk(ins));
                    }
                }
                //Err(e) => println!("cserr! {:?}", e),
                Err(e) => {}
            },
            //Err(e) => println!("memerr! {:?}", e),
            Err(e) => {}
        }
    }

    //println!(
    //    "updating {:#018x} {}\n",
    //    map.start(),
    //    instructions.iter().len()
    //);

    instructions
}

pub fn get_all_disassembly(memory_maps: &[MemMap], pid: u64) -> HashMap<u64, Disassembly> {
    let cs = Capstone::new()
        .x86()
        .mode(arch::x86::ArchMode::Mode64)
        .detail(true)
        .syntax(arch::x86::ArchSyntax::Intel)
        .build()
        .unwrap();

    let mut instructions = HashMap::new();
    for map in memory_maps {
        instructions.extend(disasm_in_map(map, pid, 0).into_iter());
    }
    instructions
}

pub fn read_map_containing_address(
    addr: u64,
    memory_maps: &[MemMap],
    pid: u64,
    disasm: &mut HashMap<u64, Disassembly>,
) {
    let map = memory_maps.iter().find(|map| {
        //let addr = addr as usize - map.map_range.offset;
        let addr = addr as usize;
        map.map_range.start() <= addr && (map.map_range.start() + map.map_range.size()) > addr
    });

    if map.is_none() {
        return;
    }

    let map = map.unwrap();

    //println!("off {:#018x}\n", addr - map.map_range.start() as u64);

    disasm.extend(disasm_in_map(map, pid, 0).into_iter());
}

pub fn read_at(
    addr: u64,
    memory_maps: &[MemMap],
    pid: u64,
    disasm: &mut HashMap<u64, Disassembly>,
) {
    let map = memory_maps.iter().find(|map| {
        //let addr = addr as usize - map.map_range.offset;
        let addr = addr as usize;
        map.map_range.start() <= addr && (map.map_range.start() + map.map_range.size()) >= addr
    });

    if map.is_none() {
        return;
    }

    let map = map.unwrap();
    let offset = addr as usize - map.map_range.start();

    //println!("off {:#018x}\n", addr - map.map_range.start() as u64);

    disasm.extend(disasm_in_map(map, pid, offset as usize).into_iter());
}

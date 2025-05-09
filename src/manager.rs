// -thread-info -> pid -> /proc/pid/maps
use proc_maps::get_process_maps;
use read_process_memory::CopyAddress;

pub fn read_memory_bytes(pid: u64, addr: u64, size: u64) -> Vec<u8> {
    let h: read_process_memory::ProcessHandle = (pid as i32).try_into().unwrap();
    read_process_memory::copy_address(addr as usize, size as usize, &h).unwrap()
}

pub async fn run(mut data: crate::AppDataHandle) {
    use crate::mi2command::GdbMessage::*;
    use crate::mi2command::GdbState;
    use crate::process::StdinCommand::*;
    use crate::AppEvent::*;

    let mut main_pid = None;

    loop {
        while let Ok(cmd) = data.channels.event_rx.recv().await {
            match cmd {
                Gdb(StateUpdate(GdbState::Stopped)) => {
                    data.channels.gdb_stdin_tx.send(GetRegisterUpdates).unwrap();

                    data.channels
                        .gdb_stdin_tx
                        .send(GetDisassemblyRel(32, 128))
                        .unwrap();

                    data.channels.gdb_stdin_tx.send(ThreadInfo).unwrap();
                    data.channels.gdb_stdin_tx.send(ListStackFrames).unwrap();

                    if main_pid.is_some() {
                        let pid = main_pid.unwrap();
                        if let Ok(maps) = get_process_maps(pid as i32) {
                            data.channels
                                .event_tx
                                .send(Gdb(Maps(
                                    maps.into_iter()
                                        .map(|m| crate::mi2command::MemMap { map_range: m })
                                        .collect(),
                                )))
                                .unwrap();
                        }
                    }
                }
                Gdb(UpdatedRegisters(ids)) => {
                    data.channels
                        .gdb_stdin_tx
                        .send(GetRegisterValues(ids))
                        .unwrap();
                }
                Gdb(Pid(pid)) => {
                    main_pid = Some(pid);
                    if let Ok(maps) = get_process_maps(pid as i32) {
                        data.channels
                            .event_tx
                            .send(Gdb(Maps(
                                maps.into_iter()
                                    .map(|m| crate::mi2command::MemMap { map_range: m })
                                    .collect(),
                            )))
                            .unwrap();
                    }
                }
                //ReadMemory(addr, size) => {
                //    if let Some(pid) = main_pid {
                //        let mem = read_memory_bytes(pid, addr, size);

                //        data.channels
                //            .event_tx
                //            .send(crate::AppEvent::Memory(addr, mem))
                //            .unwrap();
                //    }
                //}
                _ => {}
            }
        }
    }
}

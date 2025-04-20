// -thread-info -> pid -> /proc/pid/maps
use proc_maps::get_process_maps;

pub async fn run(mut data: crate::AppDataHandle) {
    use crate::mi2command::GdbMessage::*;
    use crate::mi2command::GdbState;
    use crate::process::StdinCommand::*;
    use crate::AppEvent::*;
    loop {
        while let Ok(cmd) = data.channels.event_rx.recv().await {
            match cmd {
                Gdb(StateUpdate(GdbState::Stopped)) => {
                    data.channels.gdb_stdin_tx.send(GetRegisterUpdates).unwrap();
                    data.channels
                        .gdb_stdin_tx
                        .send(GetDisassemblyRel(32, 128))
                        .unwrap();
                }
                Gdb(UpdatedRegisters(ids)) => {
                    data.channels
                        .gdb_stdin_tx
                        .send(GetRegisterValues(ids))
                        .unwrap();
                }
                Gdb(Pid(pid)) => {
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
                _ => {}
            }
        }
    }
}

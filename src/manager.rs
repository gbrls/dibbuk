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
                        .send(GetDisassemblyRel(0, 32))
                        .unwrap();
                }
                Gdb(UpdatedRegisters(ids)) => {
                    data.channels
                        .gdb_stdin_tx
                        .send(GetRegisterValues(ids))
                        .unwrap();
                }
                Gdb(Pid(pid)) => {
                    let maps = get_process_maps(pid as i32);
                    //println!("{:?}", maps)
                }
                _ => {}
            }
        }
    }
}

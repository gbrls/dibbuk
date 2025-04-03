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
                }
                Gdb(UpdatedRegisters(ids)) => {
                    data.channels
                        .gdb_stdin_tx
                        .send(GetRegisterValues(ids))
                        .unwrap();
                }
                _ => {}
            }
        }
    }
}

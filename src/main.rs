mod elf;
mod gdb;
mod parser;
mod tui;

#[tokio::main]
async fn main() {
    let (gdb_command_tx, gdb_command_rx) = tokio::sync::mpsc::unbounded_channel();
    let (gdb_output_tx, _) = tokio::sync::broadcast::channel(16);

    // 1. Initial command sender (fire-and-forget)
    tokio::spawn({
        let gdb_command_tx = gdb_command_tx.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let start_cmds = vec![
                gdb::GdbCommand::AddBreakpoint("main".into()),
                gdb::GdbCommand::Run,
                gdb::GdbCommand::GetRegisterNames,
                gdb::GdbCommand::GetRegisterValues,
            ];

            for cmd in start_cmds {
                gdb_command_tx.send(cmd).unwrap();
            }
        }
    });

    // 2. GDB event loop (main processor)
    let event_loop_task = tokio::spawn(gdb::run_event_loop(gdb_command_rx, gdb_output_tx.clone()));

    // 3. Output listener (with proper broadcast subscription)
    let output_task = tokio::spawn(async move {
        let mut rx = gdb_output_tx.subscribe();
        while let Ok(event) = rx.recv().await {
            println!("{:?}", event);
        }
        println!("output rx done");
    });

    // 4. Shutdown handler
    tokio::select! {
        _ = event_loop_task => {},
        _ = output_task => {},
        _ = tokio::signal::ctrl_c() => {
            println!("Shutting down...");
            gdb_command_tx.send(gdb::GdbCommand::Quit).unwrap();
        }
    }
}

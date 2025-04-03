mod app;
mod elf;
mod gdb;
mod parser;
mod tui;

#[tokio::main]
async fn main() {
    let (gdb_command_tx, gdb_command_rx) = tokio::sync::mpsc::unbounded_channel();
    let (gdb_output_tx, _) = tokio::sync::broadcast::channel(16);

    let ctx_ref = std::sync::Arc::new(tokio::sync::RwLock::new(app::DibbukState::new()));

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
    let gdb_handle = tokio::spawn(gdb::run_event_loop(gdb_command_rx, gdb_output_tx.clone()));

    let mut gdb_rx = gdb_output_tx.subscribe();
    let gdb_tx = gdb_command_tx.clone();

    let app_handle = tokio::spawn(app::DibbukState::run(ctx_ref.clone(), gdb_rx, gdb_tx));

    // 3. Output listener (with proper broadcast subscription)
    //let gdb_printer_handle = tokio::spawn(async move {
    //    while let Ok(event) = gdb_rx.recv().await {
    //        //println!("{:#?}", event.mi);
    //    }
    //    println!("output rx done");
    //});

    let gdb_tx = gdb_command_tx.clone();
    let gdb_rx = gdb_output_tx.subscribe();

    //tui::run(ctx_ref.clone(), gdb_tx, gdb_rx).await;
    let tui_handle = tokio::spawn(async move {
        tui::run(ctx_ref.clone(), gdb_tx, gdb_rx).await;
    });

    // 4. Shutdown handler
    tokio::select! {
        _ = tui_handle => {},
        _ = gdb_handle => {},
        //_ = gdb_printer_handle => {},
        //_ = tokio::signal::ctrl_c() => {
        //    println!("Shutting down...");
        //    gdb_command_tx.send(gdb::GdbCommand::Quit).unwrap();
        //}
    }
}

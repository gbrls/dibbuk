use dibbuk::{CliArgs, IOTask, TxChannels, event_loop, il};

use dibbuk::gdb;
use dibbuk::gdb::process;

use clap::{Parser, Subcommand};

#[tokio::main]
async fn main() {
    let cli = CliArgs::parse();
    // console_subscriber::init();
    let (tx_channels, gdb_stdin_rx) = TxChannels::new(&cli);
    tokio::spawn({
        let gdb_command_tx = tx_channels.stdin_tx.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let start_cmds = vec![
                il::DebuggerCommand::AddBreakpoint("main".into()),
                il::DebuggerCommand::Raw("set disassembly-flavor intel".into()),
                il::DebuggerCommand::Raw("starti".into()),
                il::DebuggerCommand::Raw("-thread-info".into()),
                il::DebuggerCommand::Raw("-environment-pwd".into()),
                il::DebuggerCommand::GetRegisterNames,
            ];

            for cmd in start_cmds {
                // gdb_command_tx.send(cmd).unwrap();
            }
        }
    });

    let gdb_handle = gdb::Builder::new().start(gdb_stdin_rx, tx_channels.stdout_tx.clone());
    // No longer will use a task for Parsing -> Lifting gdb's mi
    let mgr_handle = tokio::spawn(event_loop::update(tx_channels.clone()));
    // let tui_handle = tokio::spawn(tui::run(tx_channels.subscribe())); // WARN: never yielded

    // 4. Shutdown handler
    tokio::select! {
        // _ = tui_handle => {},
        _ = gdb_handle => {},
        // _ = app_handle => {},
        _ = mgr_handle => {},
    }
}

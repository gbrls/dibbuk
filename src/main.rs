use dibbuk::{App, CliArgs, event_loop, tui};

use dibbuk::gdb;
use dibbuk::gdb::process::{self, StdinCommand};

use clap::{Parser, Subcommand};

#[tokio::main]
async fn main() {
    let cli = CliArgs::parse();

    //TODO: tokio debugger; add cli flags to use it
    console_subscriber::init();

    let (app, gdb_stdin_rx) = App::new(&cli);

    // initial commands to gdb
    tokio::spawn({
        let gdb_command_tx = app.gdb_stdin_tx.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let start_cmds = vec![
                StdinCommand::AddBreakpoint("main".into()),
                StdinCommand::Input("set disassembly-flavor intel".into()),
                StdinCommand::Input("starti".into()),
                StdinCommand::Input("-thread-info".into()),
                StdinCommand::Input("-environment-pwd".into()),
                StdinCommand::GetRegisterNames,
            ];

            for cmd in start_cmds {
                gdb_command_tx.send(cmd).unwrap();
            }
        }
    });

    let gdb_handle = tokio::spawn(process::run_event_loop(
        gdb_stdin_rx,
        app.gdb_mi_tx.clone(),
        app.data_handle(),
    )); // WARN: occupies a large amount of space 1536 bytes
    let app_handle = tokio::spawn(gdb::lift_mi::run(app.data_handle()));
    let mgr_handle = tokio::spawn(event_loop::update(app.data_handle()));

    //let local = tokio::task::LocalSet::new();
    //let tui_handle = local.spawn_local(tui::run(app.data_handle())); // WARN: never yielded
    let tui_handle = tokio::spawn(tui::run(app.data_handle())); // WARN: never yielded

    // 4. Shutdown handler
    tokio::select! {
        _ = tui_handle => {},
        _ = gdb_handle => {},
        _ = app_handle => {},
        _ = mgr_handle => {},
    }
}

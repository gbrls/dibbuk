mod components;
mod elf;
mod manager;
mod mi2command;
mod parser;
mod process;
mod tui;

use mi2command::GdbContext;
use mi2command::GdbMessage;

use tokio::sync::broadcast;
use tokio::sync::mpsc;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use clap::builder::styling;
use clap::builder::Styles;

fn my_styles() -> Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .usage(styling::AnsiColor::Green.on_default() | styling::Effects::BOLD)
        .literal(styling::AnsiColor::Blue.on_default() | styling::Effects::BOLD)
        .placeholder(styling::AnsiColor::Cyan.on_default())
}

#[derive(Parser, Debug, Clone)]
#[command(styles(my_styles()))]
pub struct CliArgs {
    /// ELF File to be debugged
    #[arg(short, long)]
    file: Option<PathBuf>,
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

#[derive(Debug, Clone, Eq)]
pub enum AppEvent {
    Gdb(GdbMessage),
    GdbMi(parser::MiRecord),
    Any,
}

impl PartialEq for AppEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AppEvent::Any, _) => true,
            (_, AppEvent::Any) => true,
            (AppEvent::Gdb(gdb_self), AppEvent::Gdb(gdb_other)) => gdb_self == gdb_other,
            (AppEvent::GdbMi(a), AppEvent::GdbMi(b)) => a == b,
            (_, _) => false,
        }
    }
}

impl PartialOrd for AppEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        None
    }
}

#[derive(Debug)]
pub struct AppState {
    pub gdb_ctx: GdbContext,
    pub cli_args: CliArgs,
}

#[derive(Debug)]
pub struct AppChannels {
    gdb_stdin_tx: mpsc::UnboundedSender<process::StdinCommand>,
    gdb_mi_rx: broadcast::Receiver<process::MiOutput>,
    event_tx: broadcast::Sender<AppEvent>,
    event_rx: broadcast::Receiver<AppEvent>,
}

#[derive(Debug)]
pub struct AppDataHandle {
    channels: AppChannels,
    state: std::sync::Arc<tokio::sync::RwLock<AppState>>,
}

impl AppDataHandle {
    pub fn new(
        state: std::sync::Arc<tokio::sync::RwLock<AppState>>,
        gdb_stdin_tx: mpsc::UnboundedSender<process::StdinCommand>,
        gdb_mi_rx: broadcast::Receiver<process::MiOutput>,
        event_rx: broadcast::Receiver<AppEvent>,
        event_tx: broadcast::Sender<AppEvent>,
    ) -> Self {
        Self {
            state,
            channels: AppChannels {
                gdb_stdin_tx,
                gdb_mi_rx,
                event_tx,
                event_rx,
            },
        }
    }
}

struct App {
    pub gdb_stdin_tx: mpsc::UnboundedSender<process::StdinCommand>,
    pub gdb_mi_tx: broadcast::Sender<process::MiOutput>,
    pub state: std::sync::Arc<tokio::sync::RwLock<AppState>>,
    pub event_tx: broadcast::Sender<AppEvent>,
}

impl App {
    fn new(cli: &CliArgs) -> (Self, mpsc::UnboundedReceiver<process::StdinCommand>) {
        let (gdb_stdin_tx, gdb_stdin_rx) = tokio::sync::mpsc::unbounded_channel();
        let (gdb_mi_tx, _) = tokio::sync::broadcast::channel(64);
        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        let state = std::sync::Arc::new(tokio::sync::RwLock::new(AppState {
            gdb_ctx: GdbContext::new(),
            cli_args: cli.clone(),
        }));

        (
            Self {
                gdb_stdin_tx,
                gdb_mi_tx,
                event_tx,
                state,
            },
            gdb_stdin_rx,
        )
    }

    pub fn data_handle(&self) -> AppDataHandle {
        AppDataHandle::new(
            self.state.clone(),
            self.gdb_stdin_tx.clone(),
            self.gdb_mi_tx.subscribe(),
            self.event_tx.subscribe(),
            self.event_tx.clone(),
        )
    }
}

#[tokio::main]
async fn main() {
    let cli = CliArgs::parse();

    let (app, gdb_stdin_rx) = App::new(&cli);

    // initial commands to gdb
    tokio::spawn({
        let gdb_command_tx = app.gdb_stdin_tx.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let start_cmds = vec![
                process::StdinCommand::AddBreakpoint("main".into()),
                process::StdinCommand::Input("set disassembly-flavor intel".into()),
                process::StdinCommand::Run,
                process::StdinCommand::Input("-thread-info".into()),
                process::StdinCommand::GetRegisterNames,
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
    ));
    let app_handle = tokio::spawn(mi2command::run(app.data_handle()));
    let tui_handle = tokio::spawn(tui::run(app.data_handle()));
    let mgr_handle = tokio::spawn(manager::run(app.data_handle()));

    // 4. Shutdown handler
    tokio::select! {
        _ = tui_handle => {},
        _ = gdb_handle => {},
        _ = app_handle => {},
        _ = mgr_handle => {},
    }
}

pub mod components;
pub mod elf;
pub mod manager;
pub mod mi2command;
pub mod parser;
pub mod process;
pub mod theme;
pub mod tui;
pub mod process_ui;
pub mod capstone_disassembly;


pub use mi2command::GdbContext;
pub use mi2command::GdbMessage;
pub use mi2command::StackFrame;
pub use mi2command::GdbState;
pub use mi2command::MemMap;
pub use mi2command::Disassembly;

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
    Log(String),
    GdbMi(parser::MiRecord),
    ReadMemory(u64, u64),
    Memory(u64, Vec<u8>),
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

    pub fn try_read_mem(&self, addr: u64, len: u64) {
        self.channels.event_tx.send(AppEvent::ReadMemory(addr, len)).unwrap();
    }
}

pub struct App {
    pub gdb_stdin_tx: mpsc::UnboundedSender<process::StdinCommand>,
    pub gdb_mi_tx: broadcast::Sender<process::MiOutput>,
    pub state: std::sync::Arc<tokio::sync::RwLock<AppState>>,
    pub event_tx: broadcast::Sender<AppEvent>,
}

impl App {
    pub fn new(cli: &CliArgs) -> (Self, mpsc::UnboundedReceiver<process::StdinCommand>) {
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

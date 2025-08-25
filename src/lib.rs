pub mod capstone_disassembly;
// pub mod components;
pub mod debugger;
pub mod elf;
pub mod event_loop;
pub mod gdb;
pub mod il;
pub mod process_ui;
// pub mod theme;
// pub mod tui;

use futures::channel::mpsc::UnboundedReceiver;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling;

use gdb::lift_mi::*;
use gdb::parser;
use gdb::process;

pub trait IOTask {
    fn start(
        self,
        rx: mpsc::UnboundedReceiver<String>,
        tx: broadcast::Sender<String>,
    ) -> tokio::task::JoinHandle<()>;
}

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

impl CliArgs {
    pub fn new() -> Self {
        Self {
            file: None,
            debug: 0,
        }
    }
}

#[derive(Debug, Clone, Eq)]
pub enum AppEvent {
    IL(il::DebuggerEvent),
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
            (AppEvent::IL(gdb_self), AppEvent::IL(gdb_other)) => gdb_self == gdb_other,
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
pub struct RxChannels {
    pub stdout_rx: broadcast::Receiver<String>,
    pub event_rx: broadcast::Receiver<AppEvent>,
}

#[derive(Debug, Clone)]
pub struct TxChannels {
    pub stdin_tx: mpsc::UnboundedSender<String>,
    pub stdout_tx: broadcast::Sender<String>,
    pub event_tx: broadcast::Sender<AppEvent>,
}

impl TxChannels {
    /// The stdin rx channel is exclusevly owned, this is why it's returned directly on the return value
    pub fn new(cli: &CliArgs) -> (Self, mpsc::UnboundedReceiver<String>) {
        let (gdb_stdin_tx, gdb_stdin_rx) = tokio::sync::mpsc::unbounded_channel();
        let (gdb_mi_tx, _) = tokio::sync::broadcast::channel(64);
        let (event_tx, _) = tokio::sync::broadcast::channel(64);

        (
            Self {
                stdin_tx: gdb_stdin_tx,
                stdout_tx: gdb_mi_tx,
                event_tx,
            },
            gdb_stdin_rx,
        )
    }

    pub fn subscribe(&self) -> RxChannels {
        RxChannels {
            stdout_rx: self.stdout_tx.subscribe(),
            event_rx: self.event_tx.subscribe(),
        }
    }
}

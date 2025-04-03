mod elf;
mod manager;
mod mi2command;
mod parser;
mod process;
mod tui;

use tokio::sync::broadcast;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Gdb(mi2command::GdbMessage),
}

#[derive(Debug)]
pub struct AppState {
    pub gdb_ctx: mi2command::GdbContext,
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
    fn new() -> (Self, mpsc::UnboundedReceiver<process::StdinCommand>) {
        let (gdb_stdin_tx, gdb_stdin_rx) = tokio::sync::mpsc::unbounded_channel();
        let (gdb_mi_tx, _) = tokio::sync::broadcast::channel(16);
        let (event_tx, _) = tokio::sync::broadcast::channel(16);
        let state = std::sync::Arc::new(tokio::sync::RwLock::new(AppState {
            gdb_ctx: mi2command::GdbContext::new(),
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
    let (app, gdb_stdin_rx) = App::new();

    // initial commands to gdb
    tokio::spawn({
        let gdb_command_tx = app.gdb_stdin_tx.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let start_cmds = vec![
                process::StdinCommand::AddBreakpoint("main".into()),
                process::StdinCommand::Run,
                process::StdinCommand::GetRegisterNames,
            ];

            for cmd in start_cmds {
                gdb_command_tx.send(cmd).unwrap();
            }
        }
    });

    let gdb_handle = tokio::spawn(process::run_event_loop(gdb_stdin_rx, app.gdb_mi_tx.clone()));
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

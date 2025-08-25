use anyhow::Result;
use futures::{FutureExt, StreamExt};
use std::ops::AddAssign;
use tokio::sync::mpsc;

use crate::{
    IOTask,
    gdb::{
        self,
        lift_mi::{GdbLifterContext, LiftError},
    },
    il::{self, DebuggerCommand, DebuggerEvent},
};
use color_eyre::owo_colors::OwoColorize;
use log::{info, warn};

#[derive(Debug, Clone)]
struct CommandTransaction {
    pub command: DebuggerCommand,
    pub finished: Box<fn(&DebuggerEvent) -> bool>,
    pub started: Option<std::time::Instant>,
}

pub struct SessionHandle {}

#[derive(Debug)]
struct Session {}

impl Session {
    pub fn spawn<B: IOTask>(mut backend: B) -> SessionHandle {
        let (stdin_tx, stdin_rx) = tokio::sync::mpsc::unbounded_channel();
        let (stdout_tx, _) = tokio::sync::broadcast::channel(64);
        let io = backend.start(stdin_rx, stdout_tx);

        SessionHandle {}
    }
}

/// This struct is used to coordinate IO to the debugger process. If you're
/// using async code, it's best to use the `Session` and `SessionHandle` instead.
#[derive(Debug)]
struct State {
    tx: mpsc::UnboundedSender<String>,
    commands_queue: std::collections::VecDeque<CommandTransaction>,
    completed_transactions: Vec<(std::time::Instant, CommandTransaction)>,

    debugger_events_history: Vec<DebuggerEvent>,

    pub lifter: GdbLifterContext,
}

impl State {
    pub fn new(tx: mpsc::UnboundedSender<String>) -> Self {
        State {
            tx: tx,
            commands_queue: std::collections::VecDeque::new(),
            completed_transactions: Vec::new(),
            debugger_events_history: Vec::new(),
            lifter: GdbLifterContext::new(),
        }
    }

    pub fn send_command(&mut self, command: &crate::il::DebuggerCommand) {
        let str = self.lifter.lower(&command).unwrap();
        self.tx.send(str).unwrap();
    }

    pub fn queue_command(
        &mut self,
        command: &crate::il::DebuggerCommand,
        update: fn(&DebuggerEvent) -> bool,
    ) {
        if self.commands_queue.is_empty() {
            self.send_command(&command);
        }
        self.commands_queue.push_back(CommandTransaction {
            command: command.clone(),
            finished: Box::new(update),
            started: Some(std::time::Instant::now()),
        });
    }

    fn push_front_command(
        &mut self,
        command: &crate::il::DebuggerCommand,
        update: fn(&DebuggerEvent) -> bool,
    ) {
        self.send_command(&command);
        self.commands_queue.push_front(CommandTransaction {
            command: command.clone(),
            finished: Box::new(update),
            started: Some(std::time::Instant::now()),
        });
    }

    pub fn pop_command_queue(&mut self) {
        let transaction = self.commands_queue.front().unwrap();
        let now = std::time::Instant::now();

        println!(
            "Done! Transaction for {:?} in {} ms",
            transaction.command,
            now.duration_since(transaction.started.unwrap()).as_millis()
        );

        self.completed_transactions.push((now, transaction.clone()));
        self.commands_queue.pop_front();

        if !self.commands_queue.is_empty() {
            let next_transaction = self.commands_queue.front().unwrap().clone();
            self.send_command(&next_transaction.command);
        }
    }
    fn update(&mut self, evt: &il::DebuggerEvent) {
        self.debugger_events_history.push(evt.clone());

        if self.commands_queue.front().is_some() {
            let transaction = self.commands_queue.front().unwrap();
            if (transaction.finished)(evt) {
                self.pop_command_queue();
            }
        }
    }

    fn parse(&mut self, line: &str) -> Result<DebuggerEvent, LiftError> {
        let mi = gdb::parser::parse_mi_line(line);
        let mi = match mi {
            Err(_) => return Err(LiftError::InvalidMI),
            Ok((_unparsed, rec)) => {
                println!("MI: {:?}", &rec);
                rec
            }
        };
        self.lifter.lift(mi)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;

    use crate::{IOTask, RxChannels};

    use super::*;

    async fn state_recv_n_events<const N: usize>(
        mut stdout: broadcast::Receiver<String>,
        mut state: State,
    ) {
        loop {
            while let Ok(line) = stdout.recv().await {
                if let Ok(evt) = state.parse(line.as_str()) {
                    println!("EVENT: {:?} -> {:?}", &line, &evt);
                    state.update(&evt);
                }
                println!("Debugger: {:?}\n", &state);

                if state.completed_transactions.len() == N {
                    return;
                }
            }
        }
    }

    #[tokio::test]
    async fn state_simple_transactions_exe_arg() {
        // let (tx, gdb_stdin_rx) = crate::TxChannels::new(&crate::CliArgs::new());
        let gdb_handle = gdb::Builder::new()
            .push_arg("/home/gbrls/ctf/2025/dice/r2uwu2s-resort/resort")
            .spawn()
            .unwrap();

        let mut state = State::new(gdb_handle.stdin_tx.clone());

        state.queue_command(&il::DebuggerCommand::StartI, |e| match e {
            DebuggerEvent::StateUpdate(il::ExecutionState::Stopped) => true,
            _ => false,
        });
        state.queue_command(&il::DebuggerCommand::GetRegisterNames, |_| true);
        state.queue_command(&il::DebuggerCommand::ThreadInfo, |e| match e {
            DebuggerEvent::Pid(_) => true,
            _ => false,
        });

        state.queue_command(&il::DebuggerCommand::GetAllRegisterValues, |e| match e {
            DebuggerEvent::RegisterValue(_) => true,
            _ => false,
        });

        let event_loop_handle = tokio::spawn(state_recv_n_events::<4>(
            gdb_handle.subscribe_stdout(),
            state,
        ));

        match tokio::time::timeout(std::time::Duration::from_millis(2000), event_loop_handle).await
        {
            Ok(Ok(_)) => {}
            Ok(Err(join_err)) => {
                panic!("gdb_handle panicked: {join_err}");
            }
            Err(_) => {
                panic!("TIMEOUT! gdb_handle did not completed transactions within 2 seconds");
            }
        }
    }

    #[tokio::test]
    async fn state_simple_transactions_exe_command() {
        let gdb_handle = gdb::Builder::new().spawn().unwrap();

        let mut state = State::new(gdb_handle.stdin_tx.clone());

        state.queue_command(
            &il::DebuggerCommand::Raw(
                "file /home/gbrls/ctf/2025/dice/r2uwu2s-resort/resort".into(),
            ),
            |_| true,
        );

        state.queue_command(&il::DebuggerCommand::StartI, |e| match e {
            DebuggerEvent::StateUpdate(il::ExecutionState::Stopped) => true,
            _ => false,
        });
        state.queue_command(&il::DebuggerCommand::GetRegisterNames, |_| true);
        state.queue_command(&il::DebuggerCommand::ThreadInfo, |e| match e {
            DebuggerEvent::Pid(_) => true,
            _ => false,
        });

        state.queue_command(&il::DebuggerCommand::GetAllRegisterValues, |e| match e {
            DebuggerEvent::RegisterValue(_) => true,
            _ => false,
        });

        let event_loop_handle = tokio::spawn(state_recv_n_events::<4>(
            gdb_handle.subscribe_stdout(),
            state,
        ));

        match tokio::time::timeout(std::time::Duration::from_millis(2000), event_loop_handle).await
        {
            Ok(Ok(_)) => {}
            Ok(Err(join_err)) => {
                panic!("gdb_handle panicked: {join_err}");
            }
            Err(_) => {
                panic!("TIMEOUT! gdb_handle did not completed transactions within 2 seconds");
            }
        }
    }

    struct TestApp {
        debugger_context: Session,
        rx: RxChannels,
        running: bool,
    }

    impl TestApp {
        pub fn new(debugger: Session, rx: RxChannels) -> Self {
            TestApp {
                debugger_context: debugger,
                running: false,
                rx: rx,
            }
        }

        pub async fn run(mut self) {
            self.running = true;
            while self.running {
                // self.handle_stdout_event().await;
            }
        }

        // TODO: update session handle
        // async fn handle_stdout_event(&mut self) {
        //     tokio::select! {
        //         line = self.rx.stdout_rx.recv().fuse() => {
        //             if let Ok(line) = line &&
        //             let Ok(evt) = self.debugger_context.parse(line.as_str()) {
        //                 println!("EVENT: {:?} -> {:?}", &line, &evt);
        //                 self.debugger_context.update(&evt);
        //             }
        //         }
        //         _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
        //                 self.debugger_context.update(&DebuggerEvent::Tick);
        //         }
        //     }
        // }
    }

    #[tokio::test]
    async fn app_with_debugger() {
        let session = Session::spawn(gdb::Builder::new());

        // let app = TestApp::new(session, tx.subscribe());
        // app.run().await;
        // let app = TestApp::new();
    }
}

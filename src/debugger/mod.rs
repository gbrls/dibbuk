use anyhow::Result;
use tokio::sync::mpsc;

pub mod test;

use crate::{
    IOTask,
    gdb::{
        self,
        lift_mi::{GdbLifterContext, LiftError},
        mi::MiRecord,
    },
    il::{self, DebuggerCommand, DebuggerEvent},
};

#[derive(Debug, Clone)]
struct CommandTransaction {
    pub command: DebuggerCommand,
    pub started: Option<std::time::Instant>,
}

pub struct SessionHandle {}

#[derive(Debug)]
struct Session {}

impl Session {
    pub fn spawn<B: IOTask>(backend: B) -> SessionHandle {
        let (stdin_tx, stdin_rx) = tokio::sync::mpsc::unbounded_channel();
        let (stdout_tx, _) = tokio::sync::broadcast::channel(64);
        let io = backend.start(stdin_rx, stdout_tx);

        SessionHandle {}
    }
}

/// This struct is used to coordinate IO to the debugger process. If you're
/// using async code, it's best to use the `Session` and `SessionHandle` instead.
#[derive(Debug)]
pub struct State {
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

    pub fn send(&mut self, command: &crate::il::DebuggerCommand) {
        let str = self.lifter.lower(&command).unwrap();
        self.tx.send(str).unwrap();
    }

    pub fn queue(&mut self, command: &DebuggerCommand) {
        if self.commands_queue.is_empty() {
            self.send(&command);
        }
        self.commands_queue.push_back(CommandTransaction {
            command: command.clone(),
            started: Some(std::time::Instant::now()),
        });
    }

    fn push_front(&mut self, command: &crate::il::DebuggerCommand) {
        self.send(&command);
        self.commands_queue.push_front(CommandTransaction {
            command: command.clone(),
            started: Some(std::time::Instant::now()),
        });
    }

    pub fn update_queue(&mut self) {
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
            self.send(&next_transaction.command);
        }
    }
    pub fn update(&mut self, evt: &il::DebuggerEvent) {
        self.debugger_events_history.push(evt.clone());

        if self.commands_queue.front().is_some() {
            let transaction = self.commands_queue.front().unwrap();
            match transaction.command.generated(evt) {
                Some(true) | None => {
                    self.update_queue();
                }
                _ => {}
            }
        }
    }

    pub fn lift(&mut self, mi: MiRecord) -> Result<DebuggerEvent, LiftError> {
        self.lifter.lift(mi)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;

    use crate::{IOTask, RxChannels, gdb::process::GdbHandle};

    use super::*;

    async fn state_recv_n_events<const N: usize>(
        mut stdout: broadcast::Receiver<String>,
        mut state: State,
    ) {
        loop {
            while let Ok(line) = stdout.recv().await {
                if let Ok(evt) = state.lift(gdb::mi::parse(line.as_str()).unwrap()) {
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

        use il::DebuggerCommand::*;
        state.queue(&StartI);
        state.queue(&GetRegisterNames);
        state.queue(&ThreadInfo);
        state.queue(&GetAllRegisterValues);

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

        state.queue(&il::DebuggerCommand::Raw(
            "file /home/gbrls/ctf/2025/dice/r2uwu2s-resort/resort".into(),
        ));

        state.queue(&il::DebuggerCommand::StartI);
        state.queue(&il::DebuggerCommand::GetRegisterNames);
        state.queue(&il::DebuggerCommand::ThreadInfo);
        state.queue(&il::DebuggerCommand::GetAllRegisterValues);

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
        debugger_state: State,
        stdout: broadcast::Receiver<String>,
        running: bool,
    }

    impl TestApp {
        pub fn new(gdb_handle: &GdbHandle) -> Self {
            TestApp {
                debugger_state: State::new(gdb_handle.stdin_tx.clone()),
                stdout: gdb_handle.subscribe_stdout(),
                running: false,
            }
        }

        pub async fn run(mut self) {
            self.running = true;
            self.debugger_state
                .queue(&DebuggerCommand::Raw("help".into()));
            self.debugger_state
                .queue(&DebuggerCommand::Raw("asd;flkajsdf".into()));

            while self.running {
                if self.handle_stdout_event().await {
                    break;
                }
            }
        }

        async fn handle_stdout_event(&mut self) -> bool {
            tokio::select! {
                line = self.stdout.recv() => {
                    if let Ok(line) = line &&
                    let Ok(evt) = self.debugger_state.lift(gdb::mi::parse(line.as_str()).unwrap()) {
                        println!("EVENT: {:?} -> {:?}", &line, &evt);
                        self.debugger_state.update(&evt);
                        true
                    } else {

                        false
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        self.debugger_state.update(&DebuggerEvent::Tick);
                        false
                }
            }
        }
    }

    #[tokio::test]
    async fn app_with_debugger() {
        let gdb_handle = gdb::Builder::new().spawn().unwrap();
        let app = TestApp::new(&gdb_handle);
        app.run().await;
        // let app = TestApp::new();
    }
}

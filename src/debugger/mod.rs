use std::ops::AddAssign;

use crate::{
    gdb::{self, lift_mi::GdbLifterContext},
    il::{self, DebuggerCommand, DebuggerEvent},
};
use color_eyre::owo_colors::OwoColorize;
use log::{info, warn};

#[derive(Debug, Clone)]
struct CommandTransaction {
    pub command: DebuggerCommand,
    pub finished: Box<fn(Option<&DebuggerEvent>) -> bool>,
    pub started: Option<std::time::Instant>,
}

#[derive(Debug)]
struct DebuggerState {
    app_handles: crate::AppDataHandle,
    commands_queue: std::collections::VecDeque<CommandTransaction>,
    completed_transactions: Vec<(std::time::Instant, CommandTransaction)>,
    pub lifter: GdbLifterContext,
}

impl DebuggerState {
    pub fn new(data: crate::AppDataHandle) -> Self {
        DebuggerState {
            app_handles: data,
            commands_queue: std::collections::VecDeque::new(),
            completed_transactions: Vec::new(),
            lifter: GdbLifterContext::new(),
        }
    }

    pub fn send_command(&mut self, command: &crate::il::DebuggerCommand) {
        let str = self.lifter.lower(&command).unwrap();
        self.app_handles.channels.stdin_tx.send(str).unwrap();
    }

    pub fn queue_command(
        &mut self,
        command: &crate::il::DebuggerCommand,
        update: fn(Option<&DebuggerEvent>) -> bool,
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

    pub fn push_front_command(
        &mut self,
        command: &crate::il::DebuggerCommand,
        update: fn(Option<&DebuggerEvent>) -> bool,
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
    pub fn update(&mut self, evt: Option<&il::DebuggerEvent>) {
        if self.commands_queue.front().is_some() {
            let transaction = self.commands_queue.front().unwrap();
            if (transaction.finished)(evt) {
                self.pop_command_queue();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_task<const N: usize>(data: crate::AppDataHandle, mut debugger: DebuggerState) {
        let mut stdout = data.channels.stdout_rx.resubscribe();

        loop {
            while let Ok(line) = stdout.recv().await {
                println!("- line: {:#?}", &line);
                let mi = gdb::parser::parse_mi_line(&line);
                let mi = match mi {
                    Err(e) => panic!("parsing error {e:?}"),
                    Ok((_unparsed, rec)) => rec,
                };
                println!("- mi: {:#?}", &mi);
                let evt = debugger.lifter.lift(mi).unwrap();
                println!("- evt: {:#?}", &evt);

                println!("Debugger: {:?}\n", &debugger);
                debugger.update(evt.as_ref());
                println!("Debugger: {:?}\n", &debugger);

                if debugger.completed_transactions.len() == N {
                    return;
                }
            }
        }
    }

    #[tokio::test]
    async fn simple_transactions() {
        let (app, gdb_stdin_rx) = crate::App::new(&crate::CliArgs::new());

        let mut debugger = DebuggerState::new(app.data_handle());

        debugger.queue_command(&il::DebuggerCommand::StartI, |e| match e {
            Some(DebuggerEvent::StateUpdate(il::ExecutionState::Stopped)) => true,
            _ => false,
        });
        debugger.queue_command(&il::DebuggerCommand::GetRegisterNames, |_| true);
        debugger.queue_command(&il::DebuggerCommand::ThreadInfo, |e| match e {
            Some(DebuggerEvent::Pid(_)) => true,
            _ => false,
        });

        debugger.queue_command(&il::DebuggerCommand::GetAllRegisterValues, |e| match e {
            Some(DebuggerEvent::RegisterValue(_)) => true,
            _ => false,
        });

        let event_loop_handle = tokio::spawn(test_task::<4>(app.data_handle(), debugger));
        let gdb_handle = tokio::spawn(crate::gdb::process::run_event_loop(
            gdb_stdin_rx,
            app.stdout_tx.clone(),
            app.data_handle(),
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
}

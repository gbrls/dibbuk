use std::collections::HashMap;

use crate::debugger::test::RecordingBuilder;
use crate::gdb::mi::MiRecord;
use crate::gdb::process::GdbHandle;
use crate::il::DebuggerCommand;
use crate::{gdb, il};
use steel::SteelVal;
use steel::rvals::SteelHashMap;
use steel::steel_vm::{engine::Engine, register_fn::RegisterFn};
use steel_derive::Steel;
use steel_repl;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;

struct AppState {
    runtime: Engine,
    stdout: broadcast::Receiver<String>,
    running: bool,
    gdb_handle: GdbHandle,
    stdin: broadcast::Receiver<String>,
}

#[derive(Debug, Clone, Steel)]
enum SteelRequest {
    Empty,
    GdbCommand(Vec<SteelVal>),
}

fn get_mi_events() -> SteelVal {
    let rec = RecordingBuilder::new()
        .path("./validation/mi-small.json")
        .load()
        .unwrap();

    let mi_vals: Vec<SteelVal> = rec
        .into_mi_test_cases()
        .into_iter()
        .map(|(_, mi)| mi.into())
        .collect();

    SteelVal::ListV(mi_vals.into())
}

impl AppState {
    pub fn new(gdb_handle: GdbHandle) -> Self {
        let (stdin_tx, stdin_rx) = broadcast::channel::<String>(16);
        let stdin_task = tokio::spawn(async move {
            let mut reader = BufReader::new(tokio::io::stdin());
            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let _ = stdin_tx.send(buf.clone());
                    }
                    Err(_) => break,
                }
            }
        });

        let mut vm = Engine::new();
        vm.register_type::<SteelRequest>("Request?");
        vm.register_fn("EmptyReq", || SteelRequest::Empty);
        vm.register_fn("GdbCommandsReq", SteelRequest::GdbCommand);
        let program = std::fs::read_to_string("./src/debugger/dibbuk.scm").unwrap();
        vm.run(program).unwrap();

        let stdout = gdb_handle.subscribe_stdout();

        AppState {
            gdb_handle,
            runtime: vm,
            running: true,
            stdout: stdout,
            stdin: stdin_rx,
        }
    }

    pub async fn run(mut self) {
        while self.running {
            self.handle_events().await;

            // let val = self.runtime.call_function_with_args(function, arguments);
        }
    }

    fn handle_stdin(&mut self, line: String) {
        // println!("Received STDIN!!!! {}", line);

        let cmd = DebuggerCommand::UserInput(line.trim_end().to_string());

        let _ = self
            .runtime
            .call_function_by_name_with_args(
                "dibbuk/handle-event",
                vec![SteelVal::Void, cmd.into()],
            )
            .unwrap();

        self.dispatch_gdb_commands();
    }

    fn handle_stdout(&mut self, line: String) {
        if let Ok(mi) = gdb::mi::parse(line.as_str()) {
            self.runtime.run("(dibbuk/hello)").unwrap();

            let _ = self
                .runtime
                .call_function_by_name_with_args(
                    "dibbuk/handle-event",
                    vec![SteelVal::Void, mi.into()],
                )
                .unwrap();
            self.dispatch_gdb_commands();
        }
    }

    fn dispatch_gdb_commands(&mut self) {
        let cmd = self
            .runtime
            .extract::<SteelRequest>("*dibbuk-command*")
            .unwrap();

        // println!("sending... {:?}", cmd);
        if let SteelRequest::GdbCommand(cmds) = cmd {
            for cmd in cmds {
                if let SteelVal::StringV(s) = cmd {
                    let cmd = format!("{}\n", s);
                    self.gdb_handle.send(cmd).unwrap();
                }
            }
        }
    }

    async fn handle_events(&mut self) {
        tokio::select! {
            Ok(line) = self.stdout.recv() => self.handle_stdout(line),
            Ok(line) = self.stdin.recv() => self.handle_stdin(line),
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(20)) => {
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{debugger::app::AppState, gdb};

    #[tokio::test]
    async fn simple_repl() {
        let gdb_handle = gdb::Builder::new()
            .push_arg("./resources/drywall")
            .spawn()
            .unwrap();
        AppState::new(gdb_handle).run().await;
    }
}

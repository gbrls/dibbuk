use std::collections::HashMap;

use crate::debugger::test::RecordingBuilder;
use crate::gdb::mi::MiRecord;
use crate::gdb::process::GdbHandle;
use crate::{gdb, il};
use steel::SteelVal;
use steel::rvals::SteelHashMap;
use steel::steel_vm::{engine::Engine, register_fn::RegisterFn};
use steel_repl;
use tokio::sync::broadcast;

struct AppState {
    runtime: Engine,
    stdout: broadcast::Receiver<String>,
    running: bool,
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
    pub fn new(gdb_handle: &GdbHandle) -> Self {
        let mut vm = Engine::new();
        let program = std::fs::read_to_string("./src/debugger/dibbuk.scm").unwrap();
        vm.run(program).unwrap();

        AppState {
            runtime: vm,
            running: true,
            stdout: gdb_handle.subscribe_stdout(),
        }
    }

    pub async fn run(mut self) {
        while self.running {
            self.handle_stdout_event().await;

            // let val = self.runtime.call_function_with_args(function, arguments);
        }
    }

    async fn handle_stdout_event(&mut self) {
        tokio::select! {
            line = self.stdout.recv() => {
                if let Ok(line) = line &&
                let Ok(mi) = gdb::mi::parse(line.as_str()) {
                    // let val = self.runtime.run("(dibbuk/hello)").unwrap();
                    let val = self.runtime.call_function_by_name_with_args("dibbuk/handle-event", vec![SteelVal::Void, mi.into()]).unwrap();
                    println!("runtime: {:?}", val);
                }
            },
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
        AppState::new(&gdb_handle).run().await;
    }
}

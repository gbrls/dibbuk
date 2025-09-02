use std::path::PathBuf;

use color_eyre::owo_colors::OwoColorize;
use steel::{SteelVal, steel_vm::register_fn::RegisterFn};
use steel_derive::Steel;

use crate::rato;

static DIBBUK_COMMAND: &'static str = "*dibbuk-command*";
static DIBBUK_STATE: &'static str = "*dibbuk-state*";

#[derive(Debug, Clone, Steel)]
pub enum RuntimeRequest {
    Empty,
    GdbCommand(Vec<SteelVal>),
    Reload,
    Term(TerminalRequest),
}

#[derive(Debug, Clone, Steel)]
pub enum TerminalRequest {
    Clear,
}

pub struct Builder {
    pub runtime_dir: String,
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            runtime_dir: "./runtime".into(),
        }
    }

    pub fn build(self) -> ScriptingRuntime {
        let mut vm = steel::steel_vm::engine::Engine::new();

        vm.register_type::<RuntimeRequest>("Request?");
        vm.register_type::<rato::TermEvent>("TermEvent?");
        vm.register_fn("EmptyReq", || RuntimeRequest::Empty);
        vm.register_fn("Reload", || RuntimeRequest::Reload);
        vm.register_fn("GdbCommandsReq", RuntimeRequest::GdbCommand);
        vm.register_fn("TermTick?", rato::TermEvent::is_tick);
        vm.register_fn("TerminalClear", || {
            RuntimeRequest::Term(TerminalRequest::Clear)
        });

        let dir: PathBuf = PathBuf::from(self.runtime_dir);

        if !dir.exists() {
            println!("DIR {dir:?} does not exist");
        }

        let mut dibbuk_main_path = dir.clone();
        dibbuk_main_path.push("dibbuk.scm");

        println!(
            "adding runtime to path {:?}",
            dibbuk_main_path.as_path().canonicalize()
        );

        let program = std::fs::read_to_string(dibbuk_main_path.as_path());
        vm.run(program.unwrap()).unwrap();

        ScriptingRuntime {
            vm: vm,
            dibbuk_main_path,
        }
    }
}

pub struct ScriptingRuntime {
    pub vm: steel::steel_vm::engine::Engine,
    pub dibbuk_main_path: PathBuf,
}

impl ScriptingRuntime {
    pub fn reload_main_with_state(&mut self, state: SteelVal) {
        let program = std::fs::read_to_string(self.dibbuk_main_path.as_path()).unwrap();
        self.vm.run(program.clone()).unwrap();
        // self.vm.register_value(DIBBUK_STATE, state);
        // NOTE: Maybe use `update_value` instead like this
        self.vm.update_value(DIBBUK_STATE, state).unwrap();
    }

    pub fn event_callback(&mut self, cmd: SteelVal) -> Option<TerminalRequest> {
        let state = self.vm.extract_value("*dibbuk-state*").unwrap();

        let res = self
            .vm
            .call_function_by_name_with_args("dibbuk/handle-event", vec![state, cmd]);

        match res {
            Ok(_) => {}
            Err(e) => {
                println!("{}", format!("Error on steel code: {:?}", e).yellow());
                if let Some(span) = e.span() {
                    let program = std::fs::read_to_string(self.dibbuk_main_path.as_path()).unwrap();
                    let ctx = 32;
                    let slice = &program
                        [(span.start() - ctx).max(0)..(span.end() + ctx).min(program.len())];
                    println!("{}", format!("-> Location\n{}", slice).red());
                }
            }
        }

        let cmd = self
            .vm
            .extract::<RuntimeRequest>("*dibbuk-command*")
            .unwrap();

        match cmd {
            RuntimeRequest::Empty => None,
            RuntimeRequest::GdbCommand(steel_vals) => None,
            RuntimeRequest::Reload => {
                let state = self.vm.extract_value("*dibbuk-state*").unwrap();
                println!("realoading state...");
                self.reload_main_with_state(state);
                println!("state reloaded!");
                None
            }
            RuntimeRequest::Term(term) => Some(term),
        }
    }

    pub fn extract_gdb_commands(&mut self) -> Vec<String> {
        let cmd = self
            .vm
            .extract::<RuntimeRequest>("*dibbuk-command*")
            .unwrap();

        let mut str_commands = vec![];
        if let RuntimeRequest::GdbCommand(cmds) = cmd {
            for cmd in cmds {
                if let SteelVal::StringV(s) = cmd {
                    let cmd = format!("{}\n", s);
                    str_commands.push(cmd);
                }
            }
        }
        str_commands
    }
}

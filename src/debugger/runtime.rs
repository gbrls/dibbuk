use std::path::PathBuf;

use color_eyre::owo_colors::OwoColorize;
use ratatui::widgets::Paragraph;
use steel::{SteelVal, steel_vm::register_fn::RegisterFn};
use steel_derive::Steel;

use crate::{
    il::MemMap,
    rato::{self, LayoutNode, RatoUI},
};

static DIBBUK_COMMAND: &'static str = "*dibbuk-command*";
static DIBBUK_STATE: &'static str = "*dibbuk-state*";
static RATO_UI: &'static str = "*rato-ui-str*";

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

// impl MapRangeImpl for MapRange {
//     fn size(&self) -> usize {
//         self.range_end - self.range_start
//     }
//     fn start(&self) -> usize {
//         self.range_start
//     }
//     fn filename(&self) -> Option<&Path> {
//         self.pathname.as_deref()
//     }
//     fn is_exec(&self) -> bool {
//         &self.flags[2..3] == "x"
//     }
//     fn is_write(&self) -> bool {
//         &self.flags[1..2] == "w"
//     }
//     fn is_read(&self) -> bool {
//         &self.flags[0..1] == "r"
//     }
// }

#[derive(Debug, Clone, Steel)]
pub struct MapRange {
    pub range_start: usize,
    pub range_end: usize,
    pub offset: usize,
    pub dev: String,
    pub flags: String,
    pub inode: usize,
    pub pathname: Option<PathBuf>,
}

impl MapRange {
    pub fn contains(&self, addr: u64) -> bool {
        self.range_start <= (addr as usize) && self.range_end >= (addr as usize)
    }
}

pub struct Builder {
    pub runtime_dir: String,
    log_vm: bool,
}

pub fn process_memory_mapping(pid: u64) -> Vec<MapRange> {
    proc_maps::get_process_maps(pid as i32)
        .unwrap()
        .into_iter()
        .map(|mp| MapRange {
            range_start: mp.start(),
            range_end: mp.start() + mp.size(),
            offset: mp.offset,
            dev: mp.dev.clone(),
            flags: mp.flags.clone(),
            inode: mp.inode,
            pathname: mp.filename().map(|p| p.to_owned()),
        })
        .collect()
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            runtime_dir: "./runtime".into(),
            log_vm: false,
        }
    }

    pub fn log(self, should: bool) -> Self {
        let mut builder = self;
        builder.log_vm = should;
        builder
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

        vm.register_type::<MapRange>("MapRange?");
        vm.register_fn("ProcessMemoryMapping", process_memory_mapping);
        vm.register_fn("MapRange->contains?", MapRange::contains);

        // vm.register_fn("Paragrah", Paragraph::new);

        let dir: PathBuf = PathBuf::from(self.runtime_dir);

        if !dir.exists() {
            println!("DIR {dir:?} does not exist");
        }

        let mut dibbuk_main_path = dir.clone();
        dibbuk_main_path.push("dibbuk.scm");

        // println!(
        // "adding runtime to path {:?}",
        // dibbuk_main_path.as_path().canonicalize()
        // );

        let program = std::fs::read_to_string(dibbuk_main_path.as_path());
        vm.run(program.unwrap()).unwrap();
        vm.update_value(
            RATO_UI,
            RatoUI {
                widgets: vec![],
                layout: LayoutNode {
                    children: vec![],
                    mode: rato::LayoutMode::Single,
                },
            }
            .into(),
        );

        ScriptingRuntime {
            vm: vm,
            dibbuk_main_path,
            log_vm: self.log_vm,
            logs: vec![],
        }
    }
}

pub struct ScriptingRuntime {
    pub vm: steel::steel_vm::engine::Engine,
    pub dibbuk_main_path: PathBuf,
    pub log_vm: bool,
    pub logs: Vec<(String, String)>,
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
            .call_function_by_name_with_args("dibbuk/handle-event", vec![state, cmd.clone()]);

        match res {
            Ok(_) => {}
            Err(e) => {
                println!("{}", format!("Error on steel code: {:?}", e).yellow());
                if let Some(span) = e.span() {
                    let program = std::fs::read_to_string(self.dibbuk_main_path.as_path()).unwrap();
                    let ctx = 32;
                    let slice = &program
                    // FIXME: this causes: (attempt to subtract with overflow)
                        [(span.start()).max(0)..(span.end() + ctx).min(program.len())];
                    println!("{}", format!("-> Location\n{}", slice).red());
                }
            }
        }

        let runtime_cmd = self.vm.extract::<RuntimeRequest>("*dibbuk-command*");
        let state = self.vm.extract_value("*dibbuk-state*").unwrap();

        if self.log_vm {
            self.logs.push((cmd.to_string(), state.to_string()));
        }

        match runtime_cmd {
            Ok(RuntimeRequest::Empty) => None,
            Ok(RuntimeRequest::GdbCommand(steel_vals)) => None,
            Ok(RuntimeRequest::Reload) => {
                println!("realoading state...");
                self.reload_main_with_state(state);
                println!("state reloaded!");
                Some(TerminalRequest::Clear)
            }
            Ok(RuntimeRequest::Term(term)) => Some(term),
            Err(_) => {
                // TODO: handle the error
                None
            }
        }
    }

    pub fn extract_rato_ui(&mut self) -> Option<rato::RatoUI> {
        let steel_val = self.vm.extract_value(RATO_UI).unwrap();
        if let SteelVal::StringV(s) = steel_val {
            let s = format!("{}", s);
            Some(s.into())
        } else {
            None
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

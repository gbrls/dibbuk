use std::{collections::HashMap, path::PathBuf};

use color_eyre::owo_colors::OwoColorize;
use ratatui::widgets::Paragraph;
use read_process_memory::ProcessHandle;
use steel::{
    SteelVal,
    rvals::{Custom, SteelHashSet},
    steel_vm::register_fn::RegisterFn,
};
use steel_derive::Steel;

use crate::{
    capstone_disassembly,
    elf::Elf,
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

#[derive(Debug, Clone)]
pub struct MapRange {
    pub range_start: usize,
    pub range_end: usize,
    pub offset: usize,
    pub dev: String,
    pub flags: String,
    pub inode: usize,
    pub pathname: Option<PathBuf>,
}

impl Custom for MapRange {
    fn fmt(&self) -> Option<std::result::Result<String, std::fmt::Error>> {
        Some(Ok(format!(
            "\n{}\nfro: {:016x}\nend: {:016x}\noff: {:x}\n\n",
            self.pathname
                .clone()
                .unwrap_or(PathBuf::new())
                .to_str()
                .unwrap(),
            self.range_start,
            self.range_end,
            self.offset
        )))
    }
}

impl MapRange {
    pub fn contains(&self, addr: u64) -> bool {
        self.range_start <= (addr as usize) && self.range_end >= (addr as usize)
    }

    pub fn flags(&self) -> String {
        self.flags.clone()
    }
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.range_end - self.range_start
    }
    pub fn start(&self) -> usize {
        self.range_start
    }
    pub fn filename(&self) -> String {
        self.pathname
            .as_deref()
            .map_or(String::new(), |p| p.as_os_str().to_str().unwrap().into())
    }
    pub fn is_exec(&self) -> bool {
        &self.flags[2..3] == "x"
    }
    pub fn is_write(&self) -> bool {
        &self.flags[1..2] == "w"
    }
    pub fn is_read(&self) -> bool {
        &self.flags[0..1] == "r"
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

#[derive(Clone, Debug)]
struct SymbolsLookup {
    sorted_arr: Vec<u64>,
    map: HashMap<u64, String>,
}

impl Custom for SymbolsLookup {
    fn fmt(&self) -> Option<std::result::Result<String, std::fmt::Error>> {
        Some(Ok(format!("{:#?}", self.map)))
    }
}

impl SymbolsLookup {
    pub fn new(path: String) -> Self {
        let elf = Elf::new(path.as_str());
        if elf.is_ok() {
            let elf = elf.unwrap();
            let mut vec: Vec<_> = elf.symbols.clone().into_keys().collect();
            vec.sort();

            SymbolsLookup {
                sorted_arr: vec,
                map: elf.symbols,
            }
        } else {
            SymbolsLookup {
                sorted_arr: Vec::new(),
                map: HashMap::new(),
            }
        }
    }

    pub fn search_symbol(&self, addr: u64) -> Option<(String, u64)> {
        if self.sorted_arr.is_empty() {
            return None;
        }

        let sym = self
            .sorted_arr
            .as_slice()
            .partition_point(|label| addr >= *label);

        if sym == self.sorted_arr.len() {
            return None;
        }

        // We want to go back one element
        let sym = if sym > 0 { sym - 1 } else { sym };

        let sym = self.sorted_arr[sym];
        let diff = addr - (sym as u64);
        self.map
            .get(&sym)
            .and_then(|str| Some((str.to_owned(), diff)))
    }
}

pub fn elf_symbols(path: String) -> HashMap<u64, String> {
    if let Ok(elf) = Elf::new(path.as_str()) {
        let mut vec: Vec<_> = elf.symbols.clone().into_keys().collect();
        vec.sort();

        elf.symbols
    } else {
        HashMap::new()
    }
}

pub fn read_proc_mem(addr: usize, length: usize, pid: u64) -> Option<Vec<u8>> {
    let handle = ProcessHandle::try_from(pid as i32).unwrap();
    read_process_memory::copy_address(addr, length, &handle).ok()
}

pub fn str_to_int(s: String, base: u32) -> Option<u64> {
    u64::from_str_radix(s.as_str(), base).ok()
}

pub fn sort(mut xs: Vec<u64>) -> Vec<u64> {
    xs.sort();
    xs.clone()
}

pub fn int_to_hex(x: u64, leading: u64) -> String {
    match leading {
        2 => format!("{:02x}", x),
        4 => format!("{:04x}", x),
        8 => format!("{:08x}", x),
        16 => format!("{:016x}", x),
        32 => format!("{:032x}", x),
        64 => format!("{:064x}", x),
        _ => format!("{:x}", x),
    }
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
        vm.register_type::<SymbolsLookup>("SymbolsLookup?");
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
        vm.register_fn("MapRange->flags", MapRange::flags);
        vm.register_fn("MapRange->filename", MapRange::filename);
        vm.register_fn("MapRange->start", MapRange::start);
        vm.register_fn("MapRange->offset", MapRange::offset);
        vm.register_fn("rust.radix-string->int", str_to_int);
        vm.register_fn("rust.int->hex", int_to_hex);
        vm.register_fn("ReadProcMem", read_proc_mem);
        vm.register_fn("DisasmMapRange", capstone_disassembly::disasm_in_map);
        vm.register_fn("DisasmMapRangeOffset", capstone_disassembly::read_at);
        vm.register_fn("rust.list->sort", sort);
        vm.register_fn("rust.elf->symbols", elf_symbols);
        vm.register_fn("rust.symbols-build", SymbolsLookup::new);
        vm.register_fn("rust.symbols-search", SymbolsLookup::search_symbol);

        // vm.register_fn("Paragrah", Paragraph::new);

        let dir: PathBuf = PathBuf::from(self.runtime_dir);

        if !dir.exists() {
            println!("DIR {dir:?} does not exist");
        }

        vm.add_search_directory(dir.clone());
        let mut dibbuk_main_path = dir.clone();
        dibbuk_main_path.push("dibbuk.scm");

        // println!(
        // "adding runtime to path {:?}",
        // dibbuk_main_path.as_path().canonicalize()
        // );

        let program = std::fs::read_to_string(dibbuk_main_path.as_path());
        // FIXME: remove this unwrap
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

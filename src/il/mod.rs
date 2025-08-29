use facet_json::{from_str, to_string};
use std::error::Error;
use steel::SteelVal;

use facet::Facet;
use steel_derive::Steel;

// Maybe seperate IL types into input (user generated) and output (gdb, capstone, ...) generated?
pub trait ILLifter<E: Error> {
    fn parse(&mut self, s: &str) -> Result<DebuggerEvent, E>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Facet, Steel)]
#[repr(u8)]
pub enum ExecutionState {
    Unknown,
    Running,
    Stopped,
    Exited,
}

impl Default for ExecutionState {
    fn default() -> Self {
        ExecutionState::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Facet, Steel)]
#[repr(u8)]
pub enum DisassemblyEngine {
    GDB,
    CS,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Facet, Steel)]
pub struct Disassembly {
    pub source: DisassemblyEngine,
    pub str: String,
    pub func: String,
    pub operand: Option<String>,
    pub mnemonic: Option<String>,
    pub offset: usize,
    pub addr: usize,
}

#[derive(Debug, Clone)]
pub struct MemMap {
    pub map_range: proc_maps::MapRange,
}

impl PartialEq for MemMap {
    fn eq(&self, other: &Self) -> bool {
        (self.map_range.start() == other.map_range.start())
            && (self.map_range.inode == other.map_range.inode)
    }
}

impl Eq for MemMap {
    fn assert_receiver_is_total_eq(&self) {}
}

impl PartialOrd for MemMap {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.map_range.start().cmp(&other.map_range.start()))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Facet, Steel)]
pub struct StackFrame {
    pub depth: u64,
    pub addr: u64,
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Facet, Steel)]
#[repr(u8)]
pub enum DebuggerEvent {
    Pid(u64),
    // TODO: TEMPORARY as the MemMap struct needs to be refactored
    // Maps(Vec<MemMap>),
    RegisterValue(Vec<(String, u64)>),
    UpdatedRegisters(Vec<String>),
    StateUpdate(ExecutionState),
    Disassembly(Vec<Disassembly>),
    StackFrames(Vec<StackFrame>),
    Cwd(String),
    Tick,
}

#[derive(Clone, Debug, PartialEq, Facet, Steel)]
#[repr(u8)]
pub enum DebuggerCommand {
    AddBreakpoint(String),
    UserInput(String),
    StepInstruction,
    StartI,
    NextInstruction,
    ListStackFrames,
    ThreadInfo,
    Finish,
    Continue,
    Run,
    InfoOs,
    GetRegisterNames,
    GetAllRegisterValues,
    GetRegisterValues(Vec<usize>),
    GetRegisterUpdates,
    GetDisassemblyRel(u64, u64),
    Quit,
}

impl DebuggerCommand {
    pub fn is_response(&self, evt: &DebuggerEvent) -> Option<bool> {
        use DebuggerCommand::*;
        use DebuggerEvent::*;
        match (self, evt) {
            (GetAllRegisterValues, RegisterValue(_)) => Some(true),
            (GetAllRegisterValues, _) => Some(false),

            (ThreadInfo, Pid(_)) => Some(true),
            (ThreadInfo, _) => Some(false),

            (StartI, StateUpdate(ExecutionState::Stopped)) => Some(true),
            (StartI, _) => Some(false),

            (_, _) => None,
        }
    }
}

impl Into<SteelVal> for DebuggerCommand {
    fn into(self) -> SteelVal {
        SteelVal::StringV(self::to_string(&self).into())
    }
}

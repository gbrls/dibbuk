// Maybe seperate IL types into input (user generated) and output (gdb, capstone, ...) generated?

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DisassemblyEngine {
    GDB,
    CS,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub struct StackFrame {
    pub depth: u64,
    pub addr: u64,
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub enum Message {
    Pid(u64),
    Maps(Vec<MemMap>),
    RegisterValue(Vec<(String, u64)>),
    UpdatedRegisters(Vec<String>),
    StateUpdate(ExecutionState),
    Disassembly(Vec<Disassembly>),
    StackFrames(Vec<StackFrame>),
    Cwd(String),
}

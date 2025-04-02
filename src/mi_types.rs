// File: src/mi_types.rs (or types.rs)

use std::collections::HashMap; // Using HashMap for key-value results for now

/// Represents a fully parsed GDB MI Record (Output Line).
#[derive(Debug, Clone, PartialEq)]
pub enum MiRecord {
    /// Result record, indicates command completion (`^`).
    Result(ResultRecord),
    /// Async record reporting execution state changes (`*`).
    ExecAsync(AsyncRecord),
    /// Async record reporting status of slow operations (`+`).
    StatusAsync(AsyncRecord),
    /// Async record reporting notifications (`=`).
    NotifyAsync(AsyncRecord),
    /// Console stream output (`~`). Text intended for direct display.
    ConsoleStream(String),
    /// Target stream output (`@`). Text coming from the target process.
    TargetStream(String),
    /// Log stream output (`&`). Text GDB wants to log.
    LogStream(String),
    /// The GDB prompt `(gdb)`. Not strictly MI but essential marker.
    GdbPrompt,
    /// Represents a line that couldn't be parsed as a known MI record.
    Unknown(String),
}

/// Represents the payload of a Result Record (`^`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResultRecord {
    /// Optional unique token sent with the command.
    pub token: Option<u64>,
    /// Result class (e.g., "done", "running", "connected", "error", "exit").
    pub class: String,
    /// Key-value pairs associated with the result.
    pub results: HashMap<String, MiValue>,
}

/// Represents the payload of an Async Record (`*`, `+`, `=`).
#[derive(Debug, Clone, PartialEq)]
pub struct AsyncRecord {
    /// Optional unique token (often present for status/exec).
    pub token: Option<u64>,
    /// The specific kind of async record based on the prefix.
    pub kind: AsyncKind,
    /// Async class (e.g., "stopped", "thread-group-added", "running").
    pub class: String,
    /// Key-value pairs associated with the async output.
    pub results: HashMap<String, MiValue>,
}

/// Distinguishes between different types of Async Records based on prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AsyncKind {
    /// Execution state change (`*`).
    Exec,
    /// Status update for slow commands (`+`).
    Status,
    /// Notification of events (`=`).
    Notify,
}

/// Represents the possible value types within MI results (key=value pairs).
/// Based on GDB MI documentation Value Types.
#[derive(Debug, Clone, PartialEq)]
pub enum MiValue {
    /// A constant string (`"content"`). Content does NOT include quotes and IS unescaped.
    Const(String),
    /// A tuple/struct (`{key="val",key=...}`). Content is an ordered map. Using Vec to preserve order.
    Tuple(Vec<(String, MiValue)>),
    /// A list (`[val1, val2, ...]`). Content is a vector of values.
    List(Vec<MiValue>),
    // Note: GDB MI also mentions C-Strings, but they seem to use the same format
    // as Const ("..."). We primarily parse Const and handle escapes within it.
    // Named lists (like `results=[key=val,...]`) are handled by parsing the outer
    // key ("results") and then its MiValue::List content.
}

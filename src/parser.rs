// File: src/mi_parser.rs (or parser.rs)

// Make sure types are accessible, e.g., using `pub mod mi_types;` in lib.rs/main.rs
// and `use super::mi_types::*;` here.

use nom::Parser;
use nom::{
    branch::alt,
    bytes::complete::{is_not, take_while1}, // Using `complete` version for simplicity now
    character::complete::{char, digit1, multispace0},
    combinator::{map, map_res, opt, recognize, value}, // `value` is useful for fixed returns
    multi::separated_list0,                            // For comma-separated results
    sequence::{delimited, preceded, separated_pair, tuple},
    IResult, // nom's standard result type: Result<(&str, Output), nom::Err>
};
use std::collections::HashMap;

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

// --- Main Entry Point ---

/// Parses a single line of GDB MI output into an `MiRecord`.
/// Assumes input does not contain the trailing newline.
pub fn parse_mi_line(input: &str) -> IResult<&str, MiRecord> {
    let input = input.trim(); // Handle potential surrounding whitespace

    // Handle the non-MI GDB prompt first
    if input == "(gdb)" {
        return Ok(("", MiRecord::GdbPrompt));
    }

    alt((
        map(parse_result_record, MiRecord::Result),
        map(parse_exec_async_record, MiRecord::ExecAsync),
        map(parse_status_async_record, MiRecord::StatusAsync),
        map(parse_notify_async_record, MiRecord::NotifyAsync),
        map(parse_console_stream, MiRecord::ConsoleStream),
        map(parse_target_stream, MiRecord::TargetStream),
        map(parse_log_stream, MiRecord::LogStream),
        // Fallback: If none match, consider it unknown
        map(recognize(nom::character::complete::anychar), |s: &str| {
            MiRecord::Unknown(s.to_string())
        }), //))(input)
    ))
    .parse(input)
}

// --- Record Type Parsers ---

// Result: ^token? class ( "," result )*
fn parse_result_record(input: &str) -> IResult<&str, ResultRecord> {
    let (i, token) = preceded(char('^'), parse_optional_token).parse(input)?;
    let (i, class) = parse_identifier(i)?;
    let (i, results) = parse_optional_results_list(i)?; // Use the results parser
    Ok((
        i,
        ResultRecord {
            token,
            class,
            results,
        },
    ))
}

// Exec Async: *token? class ( "," result )*
fn parse_exec_async_record(input: &str) -> IResult<&str, AsyncRecord> {
    let (i, token) = preceded(char('*'), parse_optional_token).parse(input)?;
    let (i, class) = parse_identifier(i)?;
    let (i, results) = parse_optional_results_list(i)?;
    Ok((
        i,
        AsyncRecord {
            token,
            kind: AsyncKind::Exec,
            class,
            results,
        },
    ))
}

// Status Async: +token? class ( "," result )*
fn parse_status_async_record(input: &str) -> IResult<&str, AsyncRecord> {
    let (i, token) = preceded(char('+'), parse_optional_token).parse(input)?;
    let (i, class) = parse_identifier(i)?;
    let (i, results) = parse_optional_results_list(i)?;
    Ok((
        i,
        AsyncRecord {
            token,
            kind: AsyncKind::Status,
            class,
            results,
        },
    ))
}

// Notify Async: =token? class ( "," result )*
fn parse_notify_async_record(input: &str) -> IResult<&str, AsyncRecord> {
    let (i, token) = preceded(char('='), parse_optional_token).parse(input)?;
    let (i, class) = parse_identifier(i)?;
    let (i, results) = parse_optional_results_list(i)?;
    Ok((
        i,
        AsyncRecord {
            token,
            kind: AsyncKind::Notify,
            class,
            results,
        },
    ))
}

// --- Stream Parsers ---

// Console: ~"content"
fn parse_console_stream(input: &str) -> IResult<&str, String> {
    preceded(char('~'), parse_mi_string_value).parse(input)
}
// Target: @"content"
fn parse_target_stream(input: &str) -> IResult<&str, String> {
    preceded(char('@'), parse_mi_string_value).parse(input)
}
// Log: &"content"
fn parse_log_stream(input: &str) -> IResult<&str, String> {
    preceded(char('&'), parse_mi_string_value).parse(input)
}

// --- Core Component Parsers ---

/// Parses the optional leading token (digits).
fn parse_optional_token(input: &str) -> IResult<&str, Option<u64>> {
    opt(map_res(digit1, |s: &str| s.parse::<u64>())).parse(input)
}

/// Parses an MI identifier (class name, variable name): letters, digits, '-', '_'
fn parse_identifier(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
        |s: &str| s.to_string(),
    )
    .parse(input)
}

/// Parses the optional comma-separated list of results following a class identifier.
/// Example: ,key1="val1",key2={...},key3=["a","b"]
fn parse_optional_results_list(input: &str) -> IResult<&str, HashMap<String, MiValue>> {
    // If no comma follows, there are no results.
    let (i, maybe_results) = opt(preceded(
        char(','),
        // Use separated_list0 for zero or more key=value pairs separated by commas
        separated_list0(char(','), parse_key_value_pair),
    ))
    .parse(input)?;

    // Convert the Vec<(String, MiValue)> from separated_list0 into a HashMap
    let results_map = match maybe_results {
        Some(pairs) => pairs.into_iter().collect(),
        None => HashMap::new(),
    };

    Ok((i, results_map))
}

/// Parses a single key=value pair.
fn parse_key_value_pair(input: &str) -> IResult<&str, (String, MiValue)> {
    separated_pair(
        parse_identifier, // The key
        char('='),
        parse_value, // The value (recursive)
    )
    .parse(input)
}

// --- Value Parsers (STUBS - Needs detailed implementation!) ---

/// Parses any MI value type (Const, Tuple, List).
/// This is the core recursive part.
fn parse_value(input: &str) -> IResult<&str, MiValue> {
    alt((
        map(parse_mi_string_value, MiValue::Const), // Handles "..."
        map(parse_tuple, MiValue::Tuple),           // Handles {...}
        map(parse_list, MiValue::List),             // Handles [...]
    ))
    .parse(input)
}

/// Parses an MI String Constant: "..."
/// This needs to handle C-style escapes properly (\", \\, \n, \t, \ooo).
fn parse_mi_string_value(input: &str) -> IResult<&str, String> {
    // !!! Placeholder: This basic version DOES NOT handle escapes !!!
    // Needs `nom::bytes::complete::escaped_transform` or similar for proper handling.
    let (i, raw_content) = delimited(
        char('"'),
        // This recognizes content *between* quotes but doesn't process escapes
        recognize(is_not("\"")), // Simpler version for now
        char('"'),
    )
    .parse(input)?;
    // TODO: Add actual unescaping logic here based on raw_content
    Ok((i, raw_content.to_string())) // Returning raw content temporarily
}

/// Parses an MI Tuple: { key=value, key=value, ... }
fn parse_tuple(input: &str) -> IResult<&str, Vec<(String, MiValue)>> {
    // Tuples are basically a list of key-value pairs inside braces
    delimited(
        char('{'),
        separated_list0(char(','), parse_key_value_pair),
        char('}'),
    )
    .parse(input)
}

/// Parses an MI List: [ value, value, ... ]
fn parse_list(input: &str) -> IResult<&str, Vec<MiValue>> {
    // Lists are values separated by commas inside square brackets
    delimited(
        char('['),
        separated_list0(char(','), parse_value), // Recursive call to parse_value
        char(']'),
    )
    .parse(input)
}

// --- Parser Tests (Place within the module or in tests/) ---
#[cfg(test)]
mod tests {
    use super::*; // Import parser functions and types
    use nom::Finish; // Needed to convert IResult to std::Result for assertions

    #[test]
    fn test_parse_prompt() {
        assert_eq!(
            parse_mi_line("(gdb)").finish().unwrap().1,
            MiRecord::GdbPrompt
        );
        assert_eq!(
            parse_mi_line("  (gdb)  ").finish().unwrap().1,
            MiRecord::GdbPrompt
        );
    }

    #[test]
    fn test_parse_done_simple() {
        let expected = MiRecord::Result(ResultRecord {
            token: None,
            class: "done".to_string(),
            results: HashMap::new(),
        });
        assert_eq!(parse_mi_line("^done").finish().unwrap().1, expected);
    }

    #[test]
    fn test_parse_done_with_token() {
        let expected = MiRecord::Result(ResultRecord {
            token: Some(123),
            class: "done".to_string(),
            results: HashMap::new(),
        });
        assert_eq!(parse_mi_line("123^done").finish().unwrap().1, expected);
    }

    #[test]
    fn test_parse_console_stream_basic() {
        // WARNING: Relies on placeholder string parsing - ignores escapes!
        let expected = MiRecord::ConsoleStream("hello world".to_string());
        assert_eq!(
            parse_mi_line("~\"hello world\"").finish().unwrap().1,
            expected
        );
    }

    // TODO: Add tests for other stream types (@, &)

    #[test]
    fn test_parse_exec_async_basic() {
        let expected = MiRecord::ExecAsync(AsyncRecord {
            token: None,
            kind: AsyncKind::Exec,
            class: "running".to_string(),
            results: HashMap::new(),
        });
        assert_eq!(parse_mi_line("*running").finish().unwrap().1, expected);
    }

    #[test]
    fn test_parse_status_async_basic() {
        let expected = MiRecord::StatusAsync(AsyncRecord {
            token: Some(45),
            kind: AsyncKind::Status,
            class: "download".to_string(),
            results: HashMap::new(),
        });
        assert_eq!(parse_mi_line("45+download").finish().unwrap().1, expected);
    }

    #[test]
    fn test_parse_notify_async_basic() {
        let expected = MiRecord::NotifyAsync(AsyncRecord {
            token: None,
            kind: AsyncKind::Notify,
            class: "thread-group-added".to_string(),
            results: HashMap::new(), // Results parsing still basic
        });
        // Example with results (will require full results parsing implementation)
        // let input = "=thread-group-added,id=\"i1\"";
        // For now, test without results:
        let input = "=thread-group-added";
        assert_eq!(parse_mi_line(input).finish().unwrap().1, expected);
    }

    // --- Tests for Value Parsing (will fail until fully implemented) ---

    // #[test]
    // fn test_parse_string_value_simple() {
    //     // Requires proper parse_mi_string_value
    //     assert_eq!(parse_value("\"simple string\"").finish().unwrap().1, MiValue::Const("simple string".to_string()));
    // }

    // #[test]
    // fn test_parse_key_value_simple() {
    //     // Requires proper parse_mi_string_value
    //      let expected_val = MiValue::Const("value1".to_string());
    //      assert_eq!(parse_key_value_pair("key1=\"value1\"").finish().unwrap().1, ("key1".to_string(), expected_val));
    // }

    // #[test]
    // fn test_parse_simple_results_list() {
    //     // Requires proper parse_key_value_pair and parse_mi_string_value
    //     let input = ",key1=\"val1\",key2=\"val2\"";
    //     let mut expected_map = HashMap::new();
    //     expected_map.insert("key1".to_string(), MiValue::Const("val1".to_string()));
    //     expected_map.insert("key2".to_string(), MiValue::Const("val2".to_string()));
    //     assert_eq!(parse_optional_results_list(input).finish().unwrap().1, expected_map);
    // }

    // #[test]
    // fn test_parse_done_with_results() {
    //     // Requires full results parsing
    //     let input = "^done,bkptno=\"1\",addr=\"0x080484a6\"";
    //     // ... construct expected ResultRecord with results HashMap ...
    //     // assert_eq!(parse_mi_line(input).finish().unwrap().1, expected);
    // }

    // TODO: Add tests for tuples {}, lists [], nested values, and escaped strings.
}

// File: src/mi_parser.rs with fixes for parsing stack frames

use nom::bytes::complete::take_while;
use nom::Parser; // Make sure this is imported

use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag, take_while1},
    character::complete::{char, digit1, multispace0, one_of}, // Added one_of for escapes
    combinator::{cut, map, map_res, opt, recognize, value},
    error::{context, ParseError}, // For adding context to errors
    multi::separated_list0,
    sequence::{delimited, preceded, separated_pair, tuple},
    Finish, // Added Finish for testing convenience
    IResult,
};
// Using HashMap as per user's provided code for top-level results
use std::collections::HashMap;

// --- Type Definitions (Merged into this file as per user request) ---

/// Represents a fully parsed GDB MI Record (Output Line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiRecord {
    Result(ResultRecord),
    ExecAsync(AsyncRecord),
    StatusAsync(AsyncRecord),
    NotifyAsync(AsyncRecord),
    ConsoleStream(String),
    TargetStream(String),
    LogStream(String),
    GdbPrompt,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultRecord {
    pub token: Option<u64>,
    pub class: String,
    /// Key-value pairs. Using HashMap based on user's last version.
    pub results: HashMap<String, MiValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncRecord {
    pub token: Option<u64>,
    pub kind: AsyncKind,
    pub class: String,
    /// Key-value pairs. Using HashMap based on user's last version.
    pub results: HashMap<String, MiValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AsyncKind {
    Exec,
    Status,
    Notify,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiValue {
    /// A constant string (`"content"`). Content is unescaped.
    Const(String),
    /// A tuple/struct (`{key="val",key=...}`). Content is an **ordered** map (Vec).
    Tuple(Vec<(String, MiValue)>), // Tuples internally preserve order
    /// A list (`[val1, val2, ...]`). Content is a vector of values.
    List(Vec<MiValue>),
}

// --- Main Parser Entry Point ---

/// Parses a single line of GDB MI output into an `MiRecord`.
/// Assumes input does not contain the trailing newline.
pub fn parse_mi_line(input: &str) -> IResult<&str, MiRecord> {
    let input = input.trim();
    if input == "(gdb)" {
        return Ok(("", MiRecord::GdbPrompt));
    }

    // Use standard nom functional style
    context(
        "MI Record",
        alt((
            map(parse_result_record, MiRecord::Result),
            map(parse_exec_async_record, MiRecord::ExecAsync),
            map(parse_status_async_record, MiRecord::StatusAsync),
            map(parse_notify_async_record, MiRecord::NotifyAsync),
            map(parse_console_stream, MiRecord::ConsoleStream),
            map(parse_target_stream, MiRecord::TargetStream),
            map(parse_log_stream, MiRecord::LogStream),
            // Fallback last - recognize consumes till end or error
            map(recognize(nom::combinator::rest), |s: &str| {
                MiRecord::Unknown(s.to_string())
            }),
        )),
    )
    .parse(input) // Apply the parser to the input here
}

// --- Record Type Parsers ---
// Reverted to standard nom chaining style, removing extraneous .parse() calls

fn parse_result_record(input: &str) -> IResult<&str, ResultRecord> {
    context("Result Record", |i| {
        let (i, token) = preceded(char('^'), parse_optional_token).parse(i)?;
        let (i, class) = parse_identifier(i)?;
        // Use cut here? Maybe not, results are optional.
        let (i, results) = parse_optional_results_list(i)?;
        Ok((
            i,
            ResultRecord {
                token,
                class,
                results,
            },
        ))
    })
    .parse(input)
}

fn parse_exec_async_record(input: &str) -> IResult<&str, AsyncRecord> {
    context("Exec Async Record", |i| {
        let (i, token) = preceded(char('*'), parse_optional_token).parse(i)?;
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
    })
    .parse(input)
}

fn parse_status_async_record(input: &str) -> IResult<&str, AsyncRecord> {
    context("Status Async Record", |i| {
        let (i, token) = preceded(char('+'), parse_optional_token).parse(i)?;
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
    })
    .parse(input)
}

fn parse_notify_async_record(input: &str) -> IResult<&str, AsyncRecord> {
    context("Notify Async Record", |i| {
        let (i, token) = preceded(char('='), parse_optional_token).parse(i)?;
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
    })
    .parse(input)
}

// --- Stream Parsers ---
fn parse_console_stream(input: &str) -> IResult<&str, String> {
    // Use cut after '~' - if we see '~', it MUST be followed by a valid string
    context(
        "Console Stream",
        preceded(char('~'), cut(parse_mi_string_value)),
    )
    .parse(input)
}
fn parse_target_stream(input: &str) -> IResult<&str, String> {
    context(
        "Target Stream",
        preceded(char('@'), cut(parse_mi_string_value)),
    )
    .parse(input)
}
fn parse_log_stream(input: &str) -> IResult<&str, String> {
    context(
        "Log Stream",
        preceded(char('&'), cut(parse_mi_string_value)),
    )
    .parse(input)
}

// --- Core Component Parsers ---
fn parse_optional_token(input: &str) -> IResult<&str, Option<u64>> {
    // This is simple enough, doesn't need context usually
    opt(map_res(digit1, |s: &str| s.parse::<u64>())).parse(input)
}

fn parse_identifier(input: &str) -> IResult<&str, String> {
    context(
        "Identifier",
        map(
            // MI Identifiers can contain letters, digits, hyphens, underscores
            take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
            |s: &str| s.to_string(),
        ),
    )
    .parse(input)
}

/// Parses the optional comma-separated list of results. Returns HashMap.
fn parse_optional_results_list(input: &str) -> IResult<&str, HashMap<String, MiValue>> {
    context("Optional Results List", |i| {
        let (i, maybe_results_vec) = opt(preceded(
            char(','),
            // Use cut: if comma is present, we expect a valid results list
            cut(separated_list0(char(','), parse_key_value_pair)),
        ))
        .parse(i)?;

        // Convert the Vec<(String, MiValue)> to HashMap as requested
        let results_map = match maybe_results_vec {
            Some(pairs) => pairs.into_iter().collect(),
            None => HashMap::new(), // Empty map if no results
        };
        Ok((i, results_map))
    })
    .parse(input)
}

/// Parses a single key=value pair.
fn parse_key_value_pair(input: &str) -> IResult<&str, (String, MiValue)> {
    // Use context and cut for better errors after '='
    context(
        "Key-Value Pair",
        separated_pair(
            parse_identifier, // Key
            cut(char('=')),   // Expect '=' after key
            parse_value,      // Value (recursive)
        ),
    )
    .parse(input)
}

// --- Value Parsers (Implemented) ---

fn parse_value(input: &str) -> IResult<&str, MiValue> {
    context(
        "MI Value",
        alt((
            map(parse_list, MiValue::List),
            map(parse_tuple, MiValue::Tuple),
            map(parse_mi_string_value, MiValue::Const),
        )),
    )
    .parse(input)
}

/// Parses an MI String Constant: "..." handling C escapes.
fn parse_mi_string_value(input: &str) -> IResult<&str, String> {
    context(
        "MI String",
        delimited(
            char('"'),
            // Use cut: If opening quote is found, expect valid string content and closing quote
            cut(opt(escaped_transform(
                // Normal characters: any char except control chars \ or "
                is_not("\\\""),
                // Control character: \
                '\\',
                // Parser for escape sequences
                alt((
                    value("\"", char('"')),  // \" -> "
                    value("\\", char('\\')), // \\ -> \
                    value("\n", char('n')),  // \n -> newline
                    value("\t", char('t')),  // \t -> tab
                    value("\r", char('r')),  // \r -> carriage return
                    value("\'", char('\'')), // \' -> ' (GDB MI spec doesn't list this but pygdbmi handles it)
                )),
            ))),
            // Expect a closing quote
            cut(char('"')),
        ),
    )
    .parse(input)
    .map(|(r, s)| {
        (
            r,
            if s.is_none() {
                String::new()
            } else {
                s.unwrap()
            },
        )
    })
}

/// Parses an MI Tuple: { key=value, key=value, ... } returns Vec to preserve internal order.
fn parse_tuple(input: &str) -> IResult<&str, Vec<(String, MiValue)>> {
    context(
        "Tuple",
        delimited(
            char('{'),
            // Use cut after opening brace
            cut(separated_list0(char(','), parse_key_value_pair)),
            // Expect closing brace
            cut(char('}')),
        ),
    )
    .parse(input)
}

/// Parses an MI List: [ value, value, ... ]
/// FIXED: This function now handles lists that contain tuples (including "frame={...}")
fn parse_list(input: &str) -> IResult<&str, Vec<MiValue>> {
    context(
        "List",
        delimited(
            char('['),
            // Use cut after opening bracket
            cut(separated_list0(
                char(','),
                // This is the key fix: Handle both direct values and key=value tuples
                alt((
                    // Handle "frame={...}" case - this is actually a "key=value" pair inside a list!
                    map(parse_key_value_pair, |(key, value)| {
                        // Convert the key-value pair to a Tuple with a single entry
                        MiValue::Tuple(vec![(key, value)])
                    }),
                    // Regular value case (recursive call)
                    parse_value,
                )),
            )),
            // Expect closing bracket
            cut(char(']')),
        ),
    )
    .parse(input)
}

// Add a test function to verify our fix works
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_frames() {
        let input = "^done,stack=[frame={level=\"0\",addr=\"0x00007ffff7a3c250\",func=\"__GI__IO_setvbuf\",file=\"iosetvbuf.c\",fullname=\"/usr/src/debug/glibc-2.40-23.fc41.x86_64/libio/iosetvbuf.c\",line=\"35\",arch=\"i386:x86-64\"},frame={level=\"1\",addr=\"0x00005555555551cc\",func=\"main\",arch=\"i386:x86-64\"}]";

        let result = parse_mi_line(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        // Check that it actually got parsed correctly
        if let Ok((_, MiRecord::Result(record))) = result {
            assert_eq!(record.class, "done");

            // Check that stack is there
            let stack = record.results.get("stack");
            assert!(stack.is_some(), "Stack not found in results");

            // Check that stack is a list with 2 frames
            if let Some(MiValue::List(frames)) = stack {
                assert_eq!(frames.len(), 2, "Expected 2 frames, got {}", frames.len());
            } else {
                panic!("Stack is not a list");
            }
        } else {
            panic!("Not a result record");
        }
    }
}


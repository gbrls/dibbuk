use facet::Facet;
use nom::Parser;

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{escaped_transform, is_not, take_while1},
    character::complete::{char, digit1},
    combinator::{cut, map, map_res, opt, recognize, value},
    error::context,
    multi::separated_list0,
    sequence::{delimited, preceded, separated_pair},
};

use std::collections::HashMap;

// Represents a fully parsed GDB MI Record (Output Line).
// source: https://sourceware.org/gdb/current/onlinedocs/gdb.html/GDB_002fMI-Output-Syntax.html#GDB_002fMI-Output-Syntax

#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
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

#[derive(Debug, Clone, PartialEq, Eq, Facet)]
pub struct ResultRecord {
    pub token: Option<u64>,
    pub class: String,
    pub results: HashMap<String, MiValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Facet)]
pub struct AsyncRecord {
    pub token: Option<u64>,
    pub kind: AsyncKind,
    pub class: String,
    pub results: HashMap<String, MiValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[repr(u8)]
pub enum AsyncKind {
    Exec,
    Status,
    Notify,
}

#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum MiValue {
    /// A constant string (`"content"`). Content is unescaped.
    Const(String),
    /// A tuple/struct (`{key="val",key=...}`). Content is an **ordered** map (Vec).
    Tuple(Vec<(String, MiValue)>), // Tuples internally preserve order
    /// A list (`[val1, val2, ...]`). Content is a vector of values.
    List(Vec<MiValue>),
}

#[derive(Copy, Debug, Clone)]
pub enum MiParseError {
    Unknown,
}

/// Parses a single line of GDB MI output into an `MiRecord`.
/// Assumes input does not contain the trailing newline.
pub fn parse(line: &str) -> Result<MiRecord, MiParseError> {
    match parse_mi_line(line) {
        Err(_) => Err(MiParseError::Unknown),
        Ok((_, mi)) => Ok(mi),
    }
}

fn parse_mi_line(input: &str) -> IResult<&str, MiRecord> {
    let input = input.trim();
    if input == "(gdb)" {
        return Ok(("", MiRecord::GdbPrompt));
    }

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
            map(recognize(nom::combinator::rest), |s: &str| {
                MiRecord::Unknown(s.to_string())
            }),
        )),
    )
    .parse(input)
}

fn parse_result_record(input: &str) -> IResult<&str, ResultRecord> {
    context("Result Record", |i| {
        let (i, token) = preceded(char('^'), parse_optional_token).parse(i)?;
        let (i, class) = parse_identifier(i)?;

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

fn parse_console_stream(input: &str) -> IResult<&str, String> {
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

fn parse_optional_token(input: &str) -> IResult<&str, Option<u64>> {
    opt(map_res(digit1, |s: &str| s.parse::<u64>())).parse(input)
}

fn parse_identifier(input: &str) -> IResult<&str, String> {
    context(
        "Identifier",
        map(
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
            cut(separated_list0(char(','), parse_key_value_pair)),
        ))
        .parse(i)?;

        let results_map = match maybe_results_vec {
            Some(pairs) => pairs.into_iter().collect(),
            None => HashMap::new(),
        };
        Ok((i, results_map))
    })
    .parse(input)
}

/// Parses a single key=value pair.
fn parse_key_value_pair(input: &str) -> IResult<&str, (String, MiValue)> {
    context(
        "Key-Value Pair",
        separated_pair(parse_identifier, cut(char('=')), parse_value),
    )
    .parse(input)
}

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
            cut(opt(escaped_transform(
                is_not("\\\""),
                '\\',
                alt((
                    value("\"", char('"')),
                    value("\\", char('\\')),
                    value("\n", char('n')),
                    value("\t", char('t')),
                    value("\r", char('r')),
                    value("\'", char('\'')),
                )),
            ))),
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
            cut(separated_list0(char(','), parse_key_value_pair)),
            cut(char('}')),
        ),
    )
    .parse(input)
}

/// Parses an MI List: [ value, value, ... ]

fn parse_list(input: &str) -> IResult<&str, Vec<MiValue>> {
    context(
        "List",
        delimited(
            char('['),
            cut(separated_list0(
                char(','),
                alt((
                    map(parse_key_value_pair, |(key, value)| {
                        MiValue::Tuple(vec![(key, value)])
                    }),
                    parse_value,
                )),
            )),
            cut(char(']')),
        ),
    )
    .parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_frames() {
        let input = "^done,stack=[frame={level=\"0\",addr=\"0x00007ffff7a3c250\",func=\"__GI__IO_setvbuf\",file=\"iosetvbuf.c\",fullname=\"/usr/src/debug/glibc-2.40-23.fc41.x86_64/libio/iosetvbuf.c\",line=\"35\",arch=\"i386:x86-64\"},frame={level=\"1\",addr=\"0x00005555555551cc\",func=\"main\",arch=\"i386:x86-64\"}]";

        let result = parse_mi_line(input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result);

        if let Ok((_, MiRecord::Result(record))) = result {
            assert_eq!(record.class, "done");

            let stack = record.results.get("stack");
            assert!(stack.is_some(), "Stack not found in results");

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

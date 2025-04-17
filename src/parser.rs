// File: src/mi_parser.rs

// User provided code with types merged into this file.
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
    //println!(
    //    ">>> Entering parse_value with input: {}",
    //    &input[..input.len().min(64)]
    //); // Print start of input
    let result = context(
        "MI Value",
        alt((
            map(parse_list, MiValue::List),
            map(parse_tuple, MiValue::Tuple),
            map(parse_mi_string_value, MiValue::Const),
        )),
    )
    .parse(input);

    match &result {
        //Ok((remaining, _)) => println!(
        //    "<<< Success parse_value, remaining: {}",
        //    &remaining[..remaining.len().min(20)]
        //),
        Err(e) => {
            //println!("<<< Failure parse_value: {:?}", e)
        }
        _ => {}
    }
    result
}

/// Parses an MI String Constant: "..." handling C escapes.
fn parse_mi_string_value(input: &str) -> IResult<&str, String> {
    //println!(">>> Entering parse_mi_string_value with input: {}", &input); // Print start of input
    context(
        "MI String",
        delimited(
            char('"'),
            //alt((tag("\""), tag("\\\""))),
            // Use cut: If opening quote is found, expect valid string content and closing quote
            opt(escaped_transform(
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
                                             // Add other C escapes if necessary (\b, \f, \v) - less common in GDB output
                                             // Octal/Hex escapes seem very uncommon in practice in MI output
                )),
            )),
            // Expect a closing quote
            cut(char('"')),
            //cut(alt((tag("\""), tag("\\\"")))),
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
fn parse_list(input: &str) -> IResult<&str, Vec<MiValue>> {
    //println!(">>> Entering parse_list with input: {}", &input); // Print start of input
    context(
        "List",
        delimited(
            char('['),
            // Use cut after opening bracket
            cut(separated_list0(char(','), parse_value)), // Recursive call
            // Expect closing bracket
            cut(char(']')),
        ),
    )
    .parse(input)
}

// --- Parser Tests (Now including value parsing) ---
#[cfg(test)]
mod tests {
    use super::*;
    use nom::Finish;
    use std::collections::HashMap; // Needed for constructing expected results

    // Helper to check parsing success and return the parsed record
    fn assert_parses_to(input: &str, expected: MiRecord) {
        match parse_mi_line(input).finish() {
            Ok(("", record)) => assert_eq!(record, expected),
            Ok((remaining, record)) => panic!(
                "Parser did not consume all input. Remaining: '{}', Parsed: {:?}",
                remaining, record
            ),
            Err(e) => {
                // Use nom_locate or nom_tracable for richer errors if needed later
                panic!("Parser failed for input '{}':\n{:#?}", input, e)
            }
        }
    }

    // Helper to check parsing failure
    fn assert_parse_fails(input: &str) {
        let result = parse_mi_line(input).finish();
        assert!(
            result.is_err(),
            "Parser unexpectedly succeeded for input: '{}'\nParsed to: {:?}",
            input,
            result.unwrap()
        );
    }

    // Helper to create HashMap for results easily in tests
    macro_rules! results_map {
        ($($key:expr => $value:expr),* $(,)?) => {
            {
                let mut map = HashMap::new();
                $(
                    map.insert($key.to_string(), $value);
                )*
                map
            }
        };
    }

    #[test]
    fn test_gdb_prompt() {
        assert_parses_to("(gdb)", MiRecord::GdbPrompt);
    }
    #[test]
    fn test_gdb_prompt_whitespace() {
        assert_parses_to("  (gdb)  ", MiRecord::GdbPrompt);
    }

    // --- Result Record Tests ---
    #[test]
    fn test_result_done_no_token_no_results() {
        assert_parses_to(
            "^done",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {},
            }),
        );
    }
    #[test]
    fn test_result_done_with_token_no_results() {
        assert_parses_to(
            "123^done",
            MiRecord::Result(ResultRecord {
                token: Some(123),
                class: "done".into(),
                results: results_map! {},
            }),
        );
    }
    #[test]
    fn test_result_running_no_token() {
        assert_parses_to(
            "^running",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "running".into(),
                results: results_map! {},
            }),
        );
    }
    #[test]
    fn test_result_error_with_token() {
        assert_parses_to(
            "4^error",
            MiRecord::Result(ResultRecord {
                token: Some(4),
                class: "error".into(),
                results: results_map! {},
            }),
        );
    }

    #[test]
    fn test_result_done_simple_results() {
        assert_parses_to(
            "5^done,name=\"value\"",
            MiRecord::Result(ResultRecord {
                token: Some(5),
                class: "done".into(),
                results: results_map! {"name" => MiValue::Const("value".into())},
            }),
        );
    }

    #[test]
    fn test_result_done_multiple_results() {
        assert_parses_to(
            "^done,name=\"val1\",num=\"123\"",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {
                    "name" => MiValue::Const("val1".into()),
                    "num" => MiValue::Const("123".into())
                },
            }),
        );
    }

    #[test]
    fn test_result_error_with_msg() {
        assert_parses_to(
            "6^error,msg=\"Something went wrong\"",
            MiRecord::Result(ResultRecord {
                token: Some(6),
                class: "error".into(),
                results: results_map! {"msg" => MiValue::Const("Something went wrong".into())},
            }),
        );
    }

    // --- Async Record Tests ---
    #[test]
    fn test_async_exec_stopped_no_results() {
        assert_parses_to(
            "*stopped",
            MiRecord::ExecAsync(AsyncRecord {
                token: None,
                kind: AsyncKind::Exec,
                class: "stopped".into(),
                results: results_map! {},
            }),
        );
    }
    #[test]
    fn test_async_exec_running_with_token() {
        assert_parses_to(
            "7*running",
            MiRecord::ExecAsync(AsyncRecord {
                token: Some(7),
                kind: AsyncKind::Exec,
                class: "running".into(),
                results: results_map! {},
            }),
        );
    }
    #[test]
    fn test_async_status_downloading() {
        assert_parses_to(
            "+download",
            MiRecord::StatusAsync(AsyncRecord {
                token: None,
                kind: AsyncKind::Status,
                class: "download".into(),
                results: results_map! {},
            }),
        );
    }
    #[test]
    fn test_async_notify_thread_created() {
        assert_parses_to(
            "=thread-created",
            MiRecord::NotifyAsync(AsyncRecord {
                token: None,
                kind: AsyncKind::Notify,
                class: "thread-created".into(),
                results: results_map! {},
            }),
        );
    }

    #[test]
    fn test_async_stopped_with_reason() {
        assert_parses_to(
            "*stopped,reason=\"breakpoint-hit\",disp=\"keep\",bkptno=\"1\"",
            MiRecord::ExecAsync(AsyncRecord {
                token: None,
                kind: AsyncKind::Exec,
                class: "stopped".into(),
                results: results_map! {
                    "reason" => MiValue::Const("breakpoint-hit".into()),
                    "disp" => MiValue::Const("keep".into()),
                    "bkptno" => MiValue::Const("1".into()),
                },
            }),
        );
    }

    #[test]
    fn test_async_notify_thread_group_added() {
        assert_parses_to(
            "=thread-group-added,id=\"i1\"",
            MiRecord::NotifyAsync(AsyncRecord {
                token: None,
                kind: AsyncKind::Notify,
                class: "thread-group-added".into(),
                results: results_map! {"id" => MiValue::Const("i1".into())},
            }),
        );
    }

    // --- Stream Record Tests ---
    #[test]
    fn test_stream_console() {
        assert_parses_to(
            "~\"Hello Console\\n\"",
            MiRecord::ConsoleStream("Hello Console\n".into()),
        );
    }
    #[test]
    fn test_stream_target() {
        assert_parses_to(
            "@\"Target says hi!\"",
            MiRecord::TargetStream("Target says hi!".into()),
        );
    }
    #[test]
    fn test_stream_log() {
        assert_parses_to(
            "&\"Log message here.\"",
            MiRecord::LogStream("Log message here.".into()),
        );
    }
    #[test]
    fn test_stream_empty() {
        assert_parses_to("~\"\"", MiRecord::ConsoleStream("".into()));
    }

    // --- String Escape Tests ---
    #[test]
    fn test_string_escapes_basic() {
        assert_parses_to(
            "~\"quote=\\\" backslash=\\\\ newline=\\n tab=\\t return=\\r\"",
            MiRecord::ConsoleStream("quote=\" backslash=\\ newline=\n tab=\t return=\r".into()),
        );
    }
    #[test]
    fn test_string_single_quote_escape() {
        assert_parses_to(
            "~\"single quote \\' test\"",
            MiRecord::ConsoleStream("single quote ' test".into()),
        );
    }

    // --- Tuple Tests ---
    #[test]
    fn test_tuple_empty() {
        assert_parses_to(
            "^done,tuple={}",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {"tuple" => MiValue::Tuple(vec![])},
            }),
        );
    }
    #[test]
    fn test_tuple_simple() {
        assert_parses_to(
            "^done,point={x=\"1\",y=\"2\"}",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {"point" => MiValue::Tuple(vec![
                    ("x".into(), MiValue::Const("1".into())),
                    ("y".into(), MiValue::Const("2".into())),
                ])},
            }),
        );
    }
    #[test]
    fn test_tuple_nested() {
        assert_parses_to(
            "^done,data={name=\"n\",val={a=\"b\"}}",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {"data" => MiValue::Tuple(vec![
                    ("name".into(), MiValue::Const("n".into())),
                    ("val".into(), MiValue::Tuple(vec![
                        ("a".into(), MiValue::Const("b".into()))
                    ]))
                ])},
            }),
        );
    }

    // --- List Tests ---
    #[test]
    fn test_list_empty() {
        assert_parses_to(
            "^done,list=[]",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {"list" => MiValue::List(vec![])},
            }),
        );
    }
    #[test]
    fn test_list_simple_const() {
        assert_parses_to(
            "^done,items=[\"a\",\"b\",\"c\"]",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {"items" => MiValue::List(vec![
                   MiValue::Const("a".into()),
                   MiValue::Const("b".into()),
                   MiValue::Const("c".into()),
                ])},
            }),
        );
    }
    #[test]
    fn test_list_mixed_types() {
        assert_parses_to(
            "^done,mixed=[\"item1\",{key=\"val\"}]",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {"mixed" => MiValue::List(vec![
                   MiValue::Const("item1".into()),
                   MiValue::Tuple(vec![("key".into(), MiValue::Const("val".into()))]), // Tuple inside list
                ])},
            }),
        );
    }

    #[test]
    fn test_list_of_lists() {
        assert_parses_to(
            "^done,matrix=[[\"1\"],[\"2\"]]",
            MiRecord::Result(ResultRecord {
                token: None,
                class: "done".into(),
                results: results_map! {"matrix" => MiValue::List(vec![
                   MiValue::List(vec![MiValue::Const("1".into())]), // List inside list
                   MiValue::List(vec![MiValue::Const("2".into())]),
                ])},
            }),
        );
    }

    // --- Complex Nested Example ---
    #[test]
    fn test_complex_nested() {
        let input = "*stopped,reason=\"breakpoint-hit\",bkptno=\"1\",frame={addr=\"0x123\",func=\"main\",args=[{name=\"argc\",value=\"1\"},{name=\"argv\",value=[\"a\",\"b\"]}]}";
        assert_parses_to(
            input,
            MiRecord::ExecAsync(AsyncRecord {
                token: None,
                kind: AsyncKind::Exec,
                class: "stopped".into(),
                results: results_map! {
                    "reason" => MiValue::Const("breakpoint-hit".into()),
                    "bkptno" => MiValue::Const("1".into()),
                    "frame" => MiValue::Tuple(vec![ // Tuple
                        ("addr".into(), MiValue::Const("0x123".into())),
                        ("func".into(), MiValue::Const("main".into())),
                        ("args".into(), MiValue::List(vec![ // List of Tuples
                            MiValue::Tuple(vec![
                                ("name".into(), MiValue::Const("argc".into())),
                                ("value".into(), MiValue::Const("1".into())),
                            ]),
                             MiValue::Tuple(vec![
                                ("name".into(), MiValue::Const("argv".into())),
                                ("value".into(), MiValue::List(vec![ // List inside Tuple inside List
                                     MiValue::Const("a".into()),
                                     MiValue::Const("b".into()),
                                ])),
                            ]),
                        ]))
                    ])
                },
            }),
        );
    }

    // --- Failure/Error Case Tests ---
    #[test]
    fn test_fail_malformed_record() {
        assert_parse_fails("?invalid");
    }
    #[test]
    fn test_fail_malformed_string() {
        assert_parse_fails("~\"unterminated");
    }
    #[test]
    fn test_fail_malformed_string_escape() {
        assert_parse_fails("~\"bad escape \\z\"");
    }
    #[test]
    fn test_fail_malformed_tuple_unclosed() {
        assert_parse_fails("^done,tuple={key=\"val\"");
    }
    #[test]
    fn test_fail_malformed_tuple_missing_equals() {
        assert_parse_fails("^done,tuple={key\"val\"}");
    }
    #[test]
    fn test_fail_malformed_list_unclosed() {
        assert_parse_fails("^done,list=[\"a\"");
    }
    #[test]
    fn test_fail_malformed_results_trailing_comma() {
        assert_parse_fails("^done,key=\"v\",");
    } // Fails because list expects value after comma
    #[test]
    fn test_fail_malformed_results_missing_comma() {
        assert_parse_fails("^done,key1=\"v1\"key2=\"v2\"");
    } // Fails because separator ',' is expected
    #[test]
    fn test_fail_incomplete_record() {
        assert_parse_fails("^");
    }
    #[test]
    fn test_fail_incomplete_string() {
        assert_parse_fails("~\"");
    }
    #[test]
    fn test_fail_incomplete_tuple() {
        assert_parse_fails("^done,a={");
    }
    #[test]
    fn test_fail_incomplete_list() {
        assert_parse_fails("^done,a=[");
    }

    #[test]
    fn test_unknown_line() {
        assert_parses_to(
            "Some random GDB text not matching MI format",
            MiRecord::Unknown("Some random GDB text not matching MI format".into()),
        );
    }
    #[test]
    fn test_only_whitespace() {
        assert_parses_to("   ", MiRecord::Unknown("   ".into()));
    } // Or handle differently if needed
      // >>> Entering parse_list with input:
    #[test]
    fn test_register_names() {
        let s = "[\"rax\",\"rbx\",\"rcx\",\"rdx\",\"rsi\",\"rdi\",\"rbp\",\"rsp\",\"r8\",\"r9\",\"r10\",\"r11\",\"r12\",\"r13\",\"r14\",\"r15\",\"rip\",\"eflags\",\"cs\",\"ss\",\"ds\",\"es\",\"fs\",\"gs\",\"st0\",\"st1\",\"st2\",\"st3\",\"st4\",\"st5\",\"st6\",\"st7\",\"fctrl\",\"fstat\",\"ftag\",\"fiseg\",\"fioff\",\"foseg\",\"fooff\",\"fop\",\"xmm0\",\"xmm1\",\"xmm2\",\"xmm3\",\"xmm4\",\"xmm5\",\"xmm6\",\"xmm7\",\"xmm8\",\"xmm9\",\"xmm10\",\"xmm11\",\"xmm12\",\"xmm13\",\"xmm14\",\"xmm15\",\"mxcsr\",\"ymm0h\",\"ymm1h\",\"ymm2h\",\"ymm3h\",\"ymm4h\",\"ymm5h\",\"ymm6h\",\"ymm7h\",\"ymm8h\",\"ymm9h\",\"ymm10h\",\"ymm11h\",\"ymm12h\",\"ymm13h\",\"ymm14h\",\"ymm15h\",\"\",\"\",\"\",\"\",\"\",\"\",\"xmm16\",\"xmm17\",\"xmm18\",\"xmm19\",\"xmm20\",\"xmm21\",\"xmm22\",\"xmm23\",\"xmm24\",\"xmm25\",\"xmm26\",\"xmm27\",\"xmm28\",\"xmm29\",\"xmm30\",\"xmm31\",\"ymm16h\",\"ymm17h\",\"ymm18h\",\"ymm19h\",\"ymm20h\",\"ymm21h\",\"ymm22h\",\"ymm23h\",\"ymm24h\",\"ymm25h\",\"ymm26h\",\"ymm27h\",\"ymm28h\",\"ymm29h\",\"ymm30h\",\"ymm31h\",\"k0\",\"k1\",\"k2\",\"k3\",\"k4\",\"k5\",\"k6\",\"k7\",\"zmm0h\",\"zmm1h\",\"zmm2h\",\"zmm3h\",\"zmm4h\",\"zmm5h\",\"zmm6h\",\"zmm7h\",\"zmm8h\",\"zmm9h\",\"zmm10h\",\"zmm11h\",\"zmm12h\",\"zmm13h\",\"zmm14h\",\"zmm15h\",\"zmm16h\",\"zmm17h\",\"zmm18h\",\"zmm19h\",\"zmm20h\",\"zmm21h\",\"zmm22h\",\"zmm23h\",\"zmm24h\",\"zmm25h\",\"zmm26h\",\"zmm27h\",\"zmm28h\",\"zmm29h\",\"zmm30h\",\"zmm31h\",\"pkru\",\"fs_base\",\"gs_base\",\"orig_rax\",\"al\",\"bl\",\"cl\",\"dl\",\"sil\",\"dil\",\"bpl\",\"spl\",\"r8l\",\"r9l\",\"r10l\",\"r11l\",\"r12l\",\"r13l\",\"r14l\",\"r15l\",\"ah\",\"bh\",\"ch\",\"dh\",\"ax\",\"bx\",\"cx\",\"dx\",\"si\",\"di\",\"bp\",\"\",\"r8w\",\"r9w\",\"r10w\",\"r11w\",\"r12w\",\"r13w\",\"r14w\",\"r15w\",\"eax\",\"ebx\",\"ecx\",\"edx\",\"esi\",\"edi\",\"ebp\",\"esp\",\"r8d\",\"r9d\",\"r10d\",\"r11d\",\"r12d\",\"r13d\",\"r14d\",\"r15d\",\"ymm0\",\"ymm1\",\"ymm2\",\"ymm3\",\"ymm4\",\"ymm5\",\"ymm6\",\"ymm7\",\"ymm8\",\"ymm9\",\"ymm10\",\"ymm11\",\"ymm12\",\"ymm13\",\"ymm14\",\"ymm15\",\"ymm16\",\"ymm17\",\"ymm18\",\"ymm19\",\"ymm20\",\"ymm21\",\"ymm22\",\"ymm23\",\"ymm24\",\"ymm25\",\"ymm26\",\"ymm27\",\"ymm28\",\"ymm29\",\"ymm30\",\"ymm31\",\"zmm0\",\"zmm1\",\"zmm2\",\"zmm3\",\"zmm4\",\"zmm5\",\"zmm6\",\"zmm7\",\"zmm8\",\"zmm9\",\"zmm10\",\"zmm11\",\"zmm12\",\"zmm13\",\"zmm14\",\"zmm15\",\"zmm16\",\"zmm17\",\"zmm18\",\"zmm19\",\"zmm20\",\"zmm21\",\"zmm22\",\"zmm23\",\"zmm24\",\"zmm25\",\"zmm26\",\"zmm27\",\"zmm28\",\"zmm29\",\"zmm30\",\"zmm31\"]";

        let r = parse_list(s);
        println!("{:?}", r);
    }
}

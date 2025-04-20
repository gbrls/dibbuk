// src/main.rs (or relevant module)

use crate::parser;
use std::io::Write; // For flushing standard streams
use std::process::Stdio;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::{
    io::{stdin, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
};

#[derive(Error, Debug)]
pub enum GdbProcessError {
    #[error("Failed to spawn GDB process: {0}")]
    SpawnError(#[from] std::io::Error),
    #[error("GDB command not found. Is GDB installed and in PATH?")]
    NotFound(std::io::Error),
    #[error("GDB process stdin handle missing")]
    MissingStdin,
    #[error("GDB process stdout handle missing")]
    MissingStdout,
    #[error("GDB process stderr handle missing")]
    MissingStderr,
}

/// Holds the handles for interacting with the GDB MI process.
pub struct GdbIo {
    pub stdin: ChildStdin,
    pub stdout_reader: BufReader<ChildStdout>,
    pub stderr_reader: BufReader<ChildStderr>,
    pub child_process: Child,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StdinCommand {
    AddBreakpoint(String),
    Input(String),
    StepInstruction,
    NextInstruction,
    Finish,
    Continue,
    Run,
    GetRegisterNames,
    GetAllRegisterValues,
    GetRegisterValues(Vec<usize>),
    GetRegisterUpdates,
    GetDisassemblyRel(u64, u64),
    Quit,
}

#[derive(Clone, Debug)]
pub enum OutputKind {
    Stdout(String),
    StdErr(String),
}

#[derive(Clone, Debug)]
pub struct MiOutput {
    pub mi: Option<parser::MiRecord>,
    pub string: OutputKind,
}

/// Spawns a GDB process configured for MI interaction. (Same as before)
pub async fn spawn_gdb_process(
    gdb_path: &str,
    cli_args: &crate::CliArgs,
) -> Result<GdbIo, GdbProcessError> {
    let mut cmd = Command::new(gdb_path);
    cmd.arg("--interpreter=mi3")
        .arg(
            cli_args
                .file
                .clone()
                .unwrap_or("/home/gbrls/ctf/2025/dice/r2uwu2s-resort/resort".into()),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--nx")
        .arg("-q");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    eprintln!("[Proxy Setup] Spawning GDB: {:?}", cmd);
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(GdbProcessError::NotFound(e));
        }
        Err(e) => {
            return Err(GdbProcessError::SpawnError(e));
        }
    };
    let pid = child.id().unwrap_or(0);
    eprintln!("[Proxy Setup] GDB process spawned (PID: {})", pid);

    let stdin = child.stdin.take().ok_or(GdbProcessError::MissingStdin)?;
    let stdout = child.stdout.take().ok_or(GdbProcessError::MissingStdout)?;
    let stderr = child.stderr.take().ok_or(GdbProcessError::MissingStderr)?;

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    Ok(GdbIo {
        stdin,
        stdout_reader,
        stderr_reader,
        child_process: child,
    })
}

pub async fn run_event_loop(
    cmd_rx: UnboundedReceiver<StdinCommand>,
    stdout_tx: tokio::sync::broadcast::Sender<MiOutput>,
    app_data: crate::AppDataHandle,
) {
    eprintln!("[Proxy Setup] Attempting to spawn GDB...");

    let cli_args = {
        let handle = app_data.state.read().await;
        handle.cli_args.clone()
    };

    let gdb_io_result = spawn_gdb_process("gdb", &cli_args).await;

    let mut gdb_io = match gdb_io_result {
        Ok(io) => io,
        Err(e) => {
            eprintln!("[Proxy Error] Failed to start GDB: {}", e);
            if matches!(e, GdbProcessError::NotFound(_)) {
                eprintln!(
                    "Hint: Check if 'gdb' is installed and accessible in your system's PATH."
                );
            }
            return; // Exit if GDB couldn't be spawned
        }
    };

    eprintln!(
        "[Proxy Info] GDB spawned (PID: {}). Proxying I/O now.",
        gdb_io.child_process.id().unwrap_or(0)
    );
    eprintln!("[Proxy Info] Type GDB MI commands below.");
    eprintln!("[Proxy Info] Type ':quit' (and press Enter) to exit the proxy cleanly.");
    eprintln!("--- GDB MI Proxy Start ---");

    // --- Task 1: Forward GDB stdout to terminal stdout ---
    // We need to move the reader into the spawned task
    let mut stdout_reader = gdb_io.stdout_reader;
    let stdout_handle = tokio::spawn(async move {
        let mut line_buf = String::new();
        loop {
            line_buf.clear();
            match stdout_reader.read_line(&mut line_buf).await {
                Ok(0) => {
                    // GDB process likely exited or closed stdout
                    println!("\n[Proxy Info] GDB stdout stream ended.");
                    break;
                }
                Ok(_) => {
                    // Print directly to the user's terminal stdout
                    //println!("received gdb command!!!!! {}", line_buf);
                    //println!("{:?}", crate::parser::parse_mi_line(&line_buf));
                    stdout_tx
                        .send(MiOutput {
                            string: OutputKind::Stdout(line_buf.clone()),
                            mi: match parser::parse_mi_line(&line_buf) {
                                Err(e) => {
                                    eprintln!("!!! {:?} -> {:?} !!!", &line_buf, e);
                                    None
                                }
                                Ok((s, rec)) => Some(rec),
                            },
                        })
                        .unwrap();
                    // Flush stdio to ensure the output is immediately visible
                    if let Err(e) = std::io::stdout().flush() {
                        eprintln!("[Proxy Warning] Failed to flush stdout: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("\n[Proxy Error] Error reading GDB stdout: {}", e);
                    break;
                }
            }
        }
    });

    // --- Task 2: Forward GDB stderr to terminal stderr ---
    // Move the reader into the spawned task
    let mut stderr_reader = gdb_io.stderr_reader;
    let stderr_handle = tokio::spawn(async move {
        let mut line_buf = String::new();
        loop {
            line_buf.clear();
            match stderr_reader.read_line(&mut line_buf).await {
                Ok(0) => {
                    // GDB stderr closed
                    // eprintln!("\n[Proxy Info] GDB stderr stream ended."); // Can be noisy
                    break;
                }
                Ok(_) => {
                    // Print directly to the user's terminal stderr
                    //eprint!("{}", line_buf);
                    // Flush stderr
                    if let Err(e) = std::io::stderr().flush() {
                        eprintln!("[Proxy Warning] Failed to flush stderr: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("\n[Proxy Error] Error reading GDB stderr: {}", e);
                    break;
                }
            }
        }
    });

    // --- Main Task: Read User Input -> Forward to GDB stdin ---
    let mut user_input_reader = BufReader::new(stdin());
    let mut gdb_stdin = gdb_io.stdin; // Take ownership of stdin writer
    let mut child_process = gdb_io.child_process; // Take ownership for waiting later

    let stdin_handle = tokio::spawn(async move {
        gdb_commands_loop(gdb_stdin, cmd_rx).await;
    });

    // --- Cleanup ---
    eprintln!("[Proxy Info] Input loop exited. Waiting for GDB process to terminate...");

    // Optional: Give GDB a moment to process the exit command before forceful measures
    // tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Wait for the GDB child process to fully exit
    match child_process.wait().await {
        Ok(status) => println!("[Proxy Info] GDB process exited with status: {}", status),
        Err(e) => eprintln!("[Proxy Error] Error waiting for GDB process: {}", e),
    }

    // Wait for the stdout/stderr forwarding tasks to complete.
    // They should complete naturally when GDB closes its output streams upon exiting.
    eprintln!("[Proxy Info] Waiting for I/O tasks to finish...");
    let (stdout_res, stderr_res, stdin_res) =
        tokio::join!(stdout_handle, stderr_handle, stdin_handle);

    if let Err(e) = stdout_res {
        eprintln!("[Proxy Warning] Stdout task panicked or failed: {}", e);
    }
    if let Err(e) = stderr_res {
        eprintln!("[Proxy Warning] Stderr task panicked or failed: {}", e);
    }

    if let Err(e) = stdin_res {
        eprintln!("[Proxy Warning] Stdin task panicked or failed: {}", e);
    }

    eprintln!("--- GDB MI Proxy End ---");
}

async fn gdb_commands_loop(mut gdb_stdin: ChildStdin, mut cmd_rx: UnboundedReceiver<StdinCommand>) {
    while let Some(cmd) = cmd_rx.recv().await {
        use StdinCommand::*;
        let mut cmd_ascii = match cmd {
            AddBreakpoint(loc) => format!("-break-insert {}", loc),
            Run => "-exec-run".into(),
            StepInstruction => "-exec-step-instruction".into(),
            NextInstruction => "-exec-next-instruction".into(),
            Finish => "-exec-finish".into(),
            Continue => "-exec-continue".into(),
            GetRegisterNames => "-data-list-register-names".into(),
            GetAllRegisterValues => "-data-list-register-values x".into(),
            GetRegisterValues(ids) => {
                let mut base = "-data-list-register-values x".to_string();
                for id in ids {
                    base.push_str(&format!(" {}", id))
                }
                base
            }
            GetRegisterUpdates => "-data-list-changed-registers".into(),
            GetDisassemblyRel(start, end) => {
                format!("-data-disassemble -s \"$pc-{}\" -e \"$pc+{}\"", start, end)
            }
            Quit => "exit".into(),
            Input(s) => s,
            _ => todo!("ops..."),
        };
        cmd_ascii.push('\n');
        //cmd_ascii.push('\n');
        if let Err(e) = gdb_stdin.write_all(cmd_ascii.as_bytes()).await {
            eprintln!("Error writing to GDB stdin: {}. Stopping command loop.", e);
            break; // Exit loop on write error
        }

        if let Err(e) = gdb_stdin.flush().await {
            eprintln!("Error flushing GDB stdin: {}. Stopping command loop.", e);
            break; // Exit loop on flush error
        }
    }
}

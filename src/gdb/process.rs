// src/main.rs (or relevant module)

use crate::gdb::parser;
use crate::il::DebuggerCommand;
use std::io::Write; // For flushing standard streams
use std::process::Stdio;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, stdin},
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

trait IoHandler {
    fn process(&mut self, stdout: String) -> Option<String>;
}

fn stdout_task(mut reader: BufReader<ChildStdout>) -> tokio::task::JoinHandle<()> {
    let handle = tokio::spawn(async move {
        let mut line_buf = String::new();
        loop {
            line_buf.clear();
            match reader.read_line(&mut line_buf).await {
                Ok(0) => {
                    println!("\n[Proxy Info] GDB stdout stream ended.");
                    break;
                }
                Ok(_) => {
                    println!("Command {line_buf:?}");
                    if line_buf.contains("(gdb)") {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("\n[Proxy Error] Error reading GDB stdout: {}", e);
                    break;
                }
            }
        }
    });
    handle
}

pub fn spawn_gdb_process(
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
    cmd_rx: UnboundedReceiver<String>,
    stdout_tx: tokio::sync::broadcast::Sender<String>,
    app_data: crate::AppDataHandle,
) {
    let cli_args = {
        let handle = app_data.state.read().await;
        handle.cli_args.clone()
    };

    let gdb_io_result = spawn_gdb_process("gdb", &cli_args);

    let gdb_io = match gdb_io_result {
        Ok(io) => io,
        Err(e) => {
            if matches!(e, GdbProcessError::NotFound(_)) {
                // TODO: err
            }
            return;
        }
    };

    eprintln!(
        "[Proxy Info] GDB spawned (PID: {}). Proxying I/O now.",
        gdb_io.child_process.id().unwrap_or(0)
    );

    let mut stdout_reader = gdb_io.stdout_reader;
    let stdout_handle = tokio::spawn(async move {
        let mut line_buf = String::new();
        loop {
            line_buf.clear();
            match stdout_reader.read_line(&mut line_buf).await {
                Ok(0) => {
                    break;
                }
                Ok(_) => {
                    stdout_tx.send(line_buf.clone()).unwrap();
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

    let mut stderr_reader = gdb_io.stderr_reader;
    let stderr_handle = tokio::spawn(async move {
        let mut line_buf = String::new();
        loop {
            line_buf.clear();
            match stderr_reader.read_line(&mut line_buf).await {
                Ok(0) => {
                    break;
                }
                Ok(_) => {
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

    let gdb_stdin = gdb_io.stdin;
    let mut child_process = gdb_io.child_process;
    let stdin_handle = tokio::spawn(async move {
        gdb_commands_loop(gdb_stdin, cmd_rx).await;
    });

    match child_process.wait().await {
        Ok(status) => println!("[Proxy Info] GDB process exited with status: {}", status),
        Err(e) => eprintln!("[Proxy Error] Error waiting for GDB process: {}", e),
    }

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

// TODO: move this to IR lowering phase
async fn gdb_commands_loop(mut gdb_stdin: ChildStdin, mut stdin_rx: UnboundedReceiver<String>) {
    while let Some(mut cmd) = stdin_rx.recv().await {
        cmd.push('\n');
        if let Err(e) = gdb_stdin.write_all(cmd.as_bytes()).await {
            eprintln!("Error writing to GDB stdin: {}. Stopping command loop.", e);
            break;
        }

        if let Err(e) = gdb_stdin.flush().await {
            eprintln!("Error flushing GDB stdin: {}. Stopping command loop.", e);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn gdb_spawn_process() {
        assert!(spawn_gdb_process("gdb", &crate::CliArgs::new()).is_ok());
    }

    #[tokio::test]
    async fn gdb_process_stdout() {
        let gdb_io = spawn_gdb_process("gdb", &crate::CliArgs::new()).unwrap();
        let mut stdout_reader = gdb_io.stdout_reader;

        // stdout_task(stdout_reader);

        let stdout_handle = tokio::spawn(async move {
            let mut line_buf = String::new();
            loop {
                line_buf.clear();
                match stdout_reader.read_line(&mut line_buf).await {
                    Ok(0) => {
                        println!("\n[Proxy Info] GDB stdout stream ended.");
                        break;
                    }
                    Ok(_) => {
                        println!("Command {line_buf:?}");
                        if line_buf.contains("(gdb)") {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("\n[Proxy Error] Error reading GDB stdout: {}", e);
                        break;
                    }
                }
            }
        });

        match timeout(Duration::from_millis(500), stdout_handle).await {
            Ok(Ok(_)) => {}
            Ok(Err(join_err)) => {
                panic!("stdout_handle task panicked: {join_err}");
            }
            Err(_) => {
                panic!("stdout_handle did not get (gdb) prompt fast enough");
            }
        }
    }
}

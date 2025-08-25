// src/main.rs (or relevant module)

use crate::gdb::parser;
use crate::il::DebuggerCommand;
use std::io::Write; // For flushing standard streams
use std::process::Stdio;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, stdin},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
};

pub struct Builder {
    executable_path: String,
    args: Vec<String>,
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            executable_path: String::new(),
            args: Vec::new(),
        }
        .gdb_path("gdb")
        .override_args(&["--interpreter=mi3", "--nx", "-q"])
    }

    pub fn override_args(mut self, args: &[&str]) -> Self {
        self.args = args.into_iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn push_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn gdb_path(mut self, path: &str) -> Self {
        self.executable_path = path.into();
        self
    }

    pub fn spawn(self) -> Result<GdbHandle, GdbProcessError> {
        let mut cmd = Command::new(self.executable_path);
        cmd.args(self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
        let (stdout_tx, stdout_rx) = broadcast::channel::<String>(1024);
        let (stderr_tx, stderr_rx) = broadcast::channel::<String>(1024);

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

        let mut stdin = child.stdin.take().ok_or(GdbProcessError::MissingStdin)?;
        let stdout = child.stdout.take().ok_or(GdbProcessError::MissingStdout)?;
        let stderr = child.stderr.take().ok_or(GdbProcessError::MissingStderr)?;

        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);

        let join = tokio::spawn(async move {
            let stdout_fut = async {
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
            };

            let stderr_fut = async {
                let mut line_buf = String::new();
                loop {
                    line_buf.clear();
                    match stderr_reader.read_line(&mut line_buf).await {
                        Ok(0) => {
                            break;
                        }
                        Ok(_) => {
                            stderr_tx.send(line_buf.clone()).unwrap();
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
            };

            let stdin_fut = async {
                while let Some(mut cmd) = stdin_rx.recv().await {
                    cmd.push('\n');
                    if let Err(e) = stdin.write_all(cmd.as_bytes()).await {
                        eprintln!("Error writing to GDB stdin: {}. Stopping command loop.", e);
                        break;
                    }

                    if let Err(e) = stdin.flush().await {
                        eprintln!("Error flushing GDB stdin: {}. Stopping command loop.", e);
                        break;
                    }
                }
            };

            let wait_fut = async {
                let _ = child.wait().await?;
                Ok::<_, std::io::Error>(())
            };

            tokio::pin!(stdin_fut, stdout_fut, stderr_fut, wait_fut);

            // Drive until shutdown or child exits.
            loop {
                tokio::select! {
                    res = &mut stdin_fut => { break; }
                    res = &mut stdout_fut => { break; }
                    res = &mut stderr_fut => { break; }
                    res = &mut wait_fut => {
                        break;
                    }
                }
            }
            Ok(())
        });

        Ok(GdbHandle {
            stdin_tx,
            stdout_rx,
            stderr_rx,
            join,
        })
    }
}

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
pub struct GdbHandle {
    pub stdin_tx: mpsc::UnboundedSender<String>,
    pub stdout_rx: broadcast::Receiver<String>,
    pub stderr_rx: broadcast::Receiver<String>,
    pub join: JoinHandle<Result<(), GdbProcessError>>,
}

impl GdbHandle {
    pub fn subscribe_stdout(&self) -> broadcast::Receiver<String> {
        self.stdout_rx.resubscribe()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    // #[tokio::test]
    // async fn gdb_spawn_process() {
    //     assert!(spawn_gdb_process("gdb", &crate::CliArgs::new()).is_ok());
    // }

    #[tokio::test]
    async fn gdb_process_stdout() {
        let gdb_handle = Builder::new().spawn().unwrap();
        let mut stdout_reader = gdb_handle.subscribe_stdout();

        // stdout_task(stdout_reader);

        let stdout_handle = tokio::spawn(async move {
            loop {
                match stdout_reader.recv().await {
                    Ok(s) if s.len() == 0 => {
                        println!("\n[Proxy Info] GDB stdout stream ended.");
                        break;
                    }
                    Ok(line) => {
                        println!("Command {line:?}");
                        if line.contains("(gdb)") {
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

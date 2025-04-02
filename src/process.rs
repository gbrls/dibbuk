// src/main.rs (or relevant module)

use std::io::Write; // For flushing standard streams
use std::process::Stdio;
use thiserror::Error;
use tokio::{
    io::{stdin, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    // No longer need select! if using separate tasks for stdout/stderr
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
    child_process: Child,
}

/// Spawns a GDB process configured for MI interaction. (Same as before)
pub async fn spawn_gdb_process(gdb_path: &str) -> Result<GdbIo, GdbProcessError> {
    let mut cmd = Command::new(gdb_path);
    cmd.arg("--interpreter=mi3")
        .arg("/home/gbrls/ctf/2025/dice/r2uwu2s-resort/resort")
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

    println!("[Proxy Setup] Spawning GDB: {:?}", cmd);
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
    println!("[Proxy Setup] GDB process spawned (PID: {})", pid);

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

pub async fn start() {
    println!("[Proxy Setup] Attempting to spawn GDB...");
    let gdb_io_result = spawn_gdb_process("gdb").await;

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

    println!(
        "[Proxy Info] GDB spawned (PID: {}). Proxying I/O now.",
        gdb_io.child_process.id().unwrap_or(0)
    );
    println!("[Proxy Info] Type GDB MI commands below.");
    println!("[Proxy Info] Type ':quit' (and press Enter) to exit the proxy cleanly.");
    println!("--- GDB MI Proxy Start ---");

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
                    println!("{}", line_buf);
                    println!("{:?}", crate::parser::parse_mi_line(&line_buf));
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
                    eprint!("{}", line_buf);
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
    let mut user_line_buf = String::new();

    use std::time::Duration;
    use tokio::time::sleep;

    sleep(Duration::from_millis(300)).await;

    let start_cmds = vec![
        "-break-insert main\n",
        "-exec-run\n",
        "-data-list-register-names\n",
        "-data-list-register-values x\n",
    ];

    for cmd in start_cmds {
        println!("(~) sending {}", cmd);
        gdb_stdin
            .write_all(cmd.as_bytes())
            .await
            .and_then(|_| Ok(gdb_stdin.flush()));
    }

    loop {
        user_line_buf.clear();
        // Read a line from the user running the proxy app
        match user_input_reader.read_line(&mut user_line_buf).await {
            Ok(0) => {
                // User pressed Ctrl+D (EOF)
                println!("\n[Proxy Info] User input EOF detected. Sending exit command to GDB.");
                // Try to tell GDB to exit gracefully
                let exit_cmd = "-gdb-exit\n";
                if let Err(e) = gdb_stdin
                    .write_all(exit_cmd.as_bytes())
                    .await
                    .and_then(|_| Ok(gdb_stdin.flush()))
                {
                    eprintln!(
                        "[Proxy Warning] Failed to send GDB exit command on user EOF: {}",
                        e
                    );
                }
                break; // Exit the input loop
            }
            Ok(_) => {
                let command_to_send = user_line_buf.trim(); // Trim whitespace

                if command_to_send == ":quit" {
                    println!(
                        "\n[Proxy Info] ':quit' command received. Sending exit command to GDB."
                    );
                    // Tell GDB to exit gracefully
                    let exit_cmd = "-gdb-exit\n";
                    if let Err(e) = gdb_stdin
                        .write_all(exit_cmd.as_bytes())
                        .await
                        .and_then(|_| Ok(gdb_stdin.flush()))
                    {
                        eprintln!("[Proxy Warning] Failed to send GDB exit command: {}", e);
                    }
                    break; // Exit the input loop
                } else if !command_to_send.is_empty() {
                    // Add newline, as GDB expects newline-terminated commands
                    let full_command = format!("{}\n", command_to_send);

                    // Forward the command to GDB's stdin
                    match gdb_stdin.write_all(full_command.as_bytes()).await {
                        Ok(_) => {
                            // Try to flush to ensure it's sent immediately
                            if let Err(e) = gdb_stdin.flush().await {
                                eprintln!("\n[Proxy Error] Failed to flush GDB stdin: {}", e);
                                // Potentially break if flushing fails, as commands might not reach GDB
                                break;
                            }
                            // Optional: Log that the command was sent
                            // println!("[Proxy Debug] Sent: {}", command_to_send);
                        }
                        Err(e) => {
                            eprintln!(
                                "\n[Proxy Error] Failed to write to GDB stdin: {}. Exiting proxy.",
                                e
                            );
                            // If we can't write to GDB, the proxy is broken
                            break;
                        }
                    }
                }
                // If the line was empty or just whitespace, do nothing and read again
            }
            Err(e) => {
                eprintln!(
                    "\n[Proxy Error] Error reading user input: {}. Exiting proxy.",
                    e
                );
                break; // Exit loop on user input error
            }
        }
    }

    // --- Cleanup ---
    println!("[Proxy Info] Input loop exited. Waiting for GDB process to terminate...");

    // Optional: Give GDB a moment to process the exit command before forceful measures
    // tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Wait for the GDB child process to fully exit
    match child_process.wait().await {
        Ok(status) => println!("[Proxy Info] GDB process exited with status: {}", status),
        Err(e) => eprintln!("[Proxy Error] Error waiting for GDB process: {}", e),
    }

    // Wait for the stdout/stderr forwarding tasks to complete.
    // They should complete naturally when GDB closes its output streams upon exiting.
    println!("[Proxy Info] Waiting for I/O tasks to finish...");
    let (stdout_res, stderr_res) = tokio::join!(stdout_handle, stderr_handle);

    if let Err(e) = stdout_res {
        eprintln!("[Proxy Warning] Stdout task panicked or failed: {}", e);
    }
    if let Err(e) = stderr_res {
        eprintln!("[Proxy Warning] Stderr task panicked or failed: {}", e);
    }

    println!("--- GDB MI Proxy End ---");
}





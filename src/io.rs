use std::process::Stdio;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    sync::{broadcast, mpsc},
    task::JoinHandle,
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[derive(Debug, Clone)]
pub struct Builder {
    executable_path: String,
    args: Vec<String>,
    start_frozen: bool,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            executable_path: String::new(),
            args: Vec::new(),
            start_frozen: false,
        }
    }

    pub fn exe(mut self, exe: impl Into<String>) -> Self {
        self.executable_path = exe.into();
        self
    }

    pub fn override_args(mut self, args: &[&str]) -> Self {
        self.args = args.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn push_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    // On Unix, child stops itself with SIGSTOP before exec so you can attach.
    pub fn frozen(mut self) -> Self {
        #[cfg(unix)]
        {
            self.start_frozen = true;
        }
        self
    }

    pub fn spawn(self) -> Result<IOHandle, IoSpawnError> {
        if self.executable_path.is_empty() {
            return Err(IoSpawnError::InvalidExe);
        }

        let mut cmd = Command::new(&self.executable_path);
        cmd.args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            cmd.process_group(0);
        }

        // #[cfg(unix)]
        // if self.start_frozen {
        //     unsafe {
        //         cmd.pre_exec(|| {
        //             let rc = libc::kill(libc::getpid(), libc::SIGSTOP);
        //             if rc != 0 {
        //                 libc::_exit(127);
        //             }
        //             Ok(())
        //         });
        //     }
        // }

        let mut child = cmd.spawn().map_err(IoSpawnError::Spawn)?;
        let pid = child.id().unwrap_or(0);
        println!("spawned process {} as PID {}", self.executable_path, pid);

        // this is a small race condition
        #[cfg(unix)]
        if self.start_frozen {
            unsafe {
                if libc::kill(pid as i32, libc::SIGSTOP) == -1 {
                    libc::_exit(127);
                }
            }
        }

        let stdin = child.stdin.take().ok_or(IoSpawnError::MissingStdin)?;
        let stdout = child.stdout.take().ok_or(IoSpawnError::MissingStdout)?;
        let stderr = child.stderr.take().ok_or(IoSpawnError::MissingStderr)?;

        let handle = IOHandle::start(child, stdin, stdout, stderr, pid);
        Ok(handle)
    }
}

#[derive(Error, Debug)]
pub enum IoSpawnError {
    #[error("invalid executable")]
    InvalidExe,
    #[error("failed to spawn: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("missing stdin")]
    MissingStdin,
    #[error("missing stdout")]
    MissingStdout,
    #[error("missing stderr")]
    MissingStderr,
    #[error("send failed: {0}")]
    Send(String),
    #[error("receive failed: {0}")]
    Recv(String),
    #[cfg(unix)]
    #[error("signal error")]
    SignalError,
}

pub struct IOHandle {
    // stdin: bounded channel for backpressure
    stdin_tx: mpsc::Sender<Vec<u8>>,
    // fan-out for stdout/stderr as bytes
    stdout_tx: broadcast::Sender<Vec<u8>>,
    stderr_tx: broadcast::Sender<Vec<u8>>,
    // for quick subscriptions
    pub stdout_rx: broadcast::Receiver<Vec<u8>>,
    pub stderr_rx: broadcast::Receiver<Vec<u8>>,
    // process control
    child: Child,
    writer_task: JoinHandle<()>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    pid: u32,
}

impl IOHandle {
    fn start(
        mut child: Child,
        stdin: ChildStdin,
        stdout: ChildStdout,
        stderr: ChildStderr,
        pid: u32,
    ) -> Self {
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<Vec<u8>>(256); // bounded for backpressure【1】
        let (stdout_tx, stdout_rx) = broadcast::channel::<Vec<u8>>(1024);
        let (stderr_tx, stderr_rx) = broadcast::channel::<Vec<u8>>(1024);

        // Writer task
        let mut writer = tokio::io::BufWriter::new(stdin);
        let writer_task = tokio::spawn(async move {
            while let Some(mut buf) = stdin_rx.recv().await {
                if writer.write_all(&buf).await.is_err() {
                    break;
                }
                if writer.flush().await.is_err() {
                    break;
                }
                buf.clear();
            }
        });

        // Stdout reader task (byte chunks)
        let mut out = stdout;
        let stdout_tx_clone = stdout_tx.clone();
        let stdout_task = tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match out.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = stdout_tx_clone.send(buf[..n].to_vec());
                    }
                    Err(_) => break,
                }
            }
        });

        // Stderr reader task (byte chunks)
        let mut err = stderr;
        let stderr_tx_clone = stderr_tx.clone();
        let stderr_task = tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                match err.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = stderr_tx_clone.send(buf[..n].to_vec());
                    }
                    Err(_) => break,
                }
            }
        });

        IOHandle {
            stdin_tx,
            stdout_tx,
            stderr_tx,
            stdout_rx,
            stderr_rx,
            child,
            writer_task,
            stdout_task,
            stderr_task,
            pid,
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn stdout(&self) -> RecvStream {
        RecvStream::new(self.stdout_tx.subscribe())
    }

    pub fn stderr(&self) -> RecvStream {
        RecvStream::new(self.stderr_tx.subscribe())
    }

    pub fn subscribe_stdout(&self) -> broadcast::Receiver<Vec<u8>> {
        self.stdout_tx.subscribe()
    }

    pub fn subscribe_stderr(&self) -> broadcast::Receiver<Vec<u8>> {
        self.stderr_tx.subscribe()
    }

    // Send raw bytes to the child.
    pub async fn send_raw(&self, bytes: impl AsRef<[u8]>) -> Result<(), IoSpawnError> {
        let v = bytes.as_ref().to_vec();
        self.stdin_tx
            .send(v)
            .await
            .map_err(|e| IoSpawnError::Send(e.to_string()))
    }

    // Convenience: send a string without newline.
    pub async fn send(&self, s: impl AsRef<str>) -> Result<(), IoSpawnError> {
        self.send_raw(s.as_ref().as_bytes()).await
    }

    // Convenience: send a string with a trailing newline.
    pub async fn send_line(&self, s: impl AsRef<str>) -> Result<(), IoSpawnError> {
        let mut v = s.as_ref().as_bytes().to_vec();
        v.push(b'\n');
        self.send_raw(v).await
    }

    pub async fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }

    #[cfg(unix)]
    pub fn resume(&self) -> Result<(), IoSpawnError> {
        let rc = unsafe { libc::kill(self.pid as libc::pid_t, libc::SIGCONT) };
        if rc == 0 {
            Ok(())
        } else {
            Err(IoSpawnError::SignalError)
        }
    }

    #[cfg(unix)]
    pub fn kill(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl Drop for IOHandle {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = self.child.start_kill();
    }
}

// Buffered receiver with pwntools-like API.
pub struct RecvStream {
    rx: broadcast::Receiver<Vec<u8>>,
    buf: Vec<u8>,
}

impl RecvStream {
    pub fn new(rx: broadcast::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            buf: Vec::new(),
        }
    }

    // Return any currently buffered bytes, or wait for the next chunk.
    pub async fn recv_some(&mut self) -> Result<Vec<u8>, IoSpawnError> {
        if !self.buf.is_empty() {
            let out = self.buf.split_off(0);
            return Ok(out);
        }
        match self.rx.recv().await {
            Ok(chunk) => Ok(chunk),
            Err(e) => Err(IoSpawnError::Recv(e.to_string())),
        }
    }

    // Read exactly n bytes, assembling across chunks.
    pub async fn recv_exact(&mut self, n: usize) -> Result<Vec<u8>, IoSpawnError> {
        while self.buf.len() < n {
            match self.rx.recv().await {
                Ok(mut chunk) => self.buf.append(&mut chunk),
                Err(e) => return Err(IoSpawnError::Recv(e.to_string())),
            }
        }
        Ok(self.buf.drain(..n).collect())
    }

    // Read until delimiter is found. If include=true, include delimiter in output.
    pub async fn recvuntil(
        &mut self,
        delim: impl AsRef<[u8]>,
        include: bool,
    ) -> Result<Vec<u8>, IoSpawnError> {
        let d = delim.as_ref();
        loop {
            if let Some(pos) = find_subslice(&self.buf, d) {
                let end = pos + d.len();
                let out = if include {
                    self.buf.drain(..end).collect()
                } else {
                    let out: Vec<u8> = self.buf.drain(..pos).collect();
                    // drop delimiter
                    self.buf.drain(..d.len());
                    out
                };
                return Ok(out);
            }
            match self.rx.recv().await {
                Ok(mut chunk) => self.buf.append(&mut chunk),
                Err(e) => return Err(IoSpawnError::Recv(e.to_string())),
            }
        }
    }
}

// Simple subslice search; consider memchr/memmem for performance.
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn io_recvuntil_and_exact_bytes() {
        // prints without newline, then later a newline
        let mut io = Builder::new()
            .exe("/bin/sh")
            .override_args(&["-c", "printf 'abc123def'; sleep 0.05; printf 'ghi\\n'"])
            .spawn()
            .expect("spawn");

        println!("started process as PID {}", io.pid());

        let mut out = io.stdout();

        let got = timeout(Duration::from_secs(1), out.recvuntil(b"123", true))
            .await
            .expect("timeout")
            .expect("recvuntil");
        assert_eq!(got, b"abc123");

        let next = timeout(Duration::from_secs(1), out.recv_exact(3))
            .await
            .expect("timeout")
            .expect("recv_exact");
        assert_eq!(next, b"def");

        let tail = timeout(Duration::from_secs(1), out.recvuntil(b"\n", false))
            .await
            .expect("timeout")
            .expect("recvuntil nl");
        assert_eq!(tail, b"ghi");

        let _ = io.wait().await;
    }

    #[tokio::test]
    async fn io_cat_binary_roundtrip() {
        let mut io = Builder::new().exe("/bin/cat").spawn().expect("spawn");
        let mut out = io.stdout();

        println!("started process as PID {}", io.pid());

        io.send_raw(&[0x00, 0xFF, 0x41]).await.expect("send");
        let data = timeout(Duration::from_millis(500), out.recv_exact(3))
            .await
            .expect("timeout")
            .expect("recv");
        assert_eq!(data, vec![0x00, 0xFF, 0x41]);

        #[cfg(unix)]
        io.kill();
        let _ = io.wait().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn io_frozen_then_resume_produces_output() {
        let mut io = Builder::new()
            .exe("/bin/sh")
            .override_args(&["-c", "printf 'frozen'"])
            .frozen()
            .spawn()
            .expect("spawn");

        println!("started process as PID {}", io.pid());

        let mut out = io.stdout();

        // // Should not get data while SIGSTOP'ed
        let res = timeout(Duration::from_millis(150), out.recv_some()).await;
        assert!(res.is_err(), "should not receive while frozen");

        io.resume().expect("resume");

        let got = timeout(Duration::from_millis(500), out.recvuntil(b"frozen", true))
            .await
            .expect("timeout")
            .expect("recv");
        assert_eq!(got, b"frozen");

        let _ = io.wait().await;
    }
}

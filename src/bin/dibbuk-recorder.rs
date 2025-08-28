// TODO: parse argv so that we can customize the recording

use dibbuk::{
    debugger::{
        queue::CommandQueue,
        test::{Recording, RecordingBuilder},
    },
    gdb::{self, Builder, mi::MiRecord, process::GdbHandle},
    il::{DebuggerCommand, DebuggerEvent},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::broadcast,
    task::JoinHandle,
};

use facet_pretty::FacetPretty;

struct TestApp {
    debugger_state: CommandQueue,
    gdb: GdbHandle, // keep the handle
    gdb_stdout: broadcast::Receiver<String>,
    stdin_task: JoinHandle<()>, // single background task we’ll cancel
    stdin_rx: broadcast::Receiver<String>,
    recording: Recording,
    running: bool,
}

impl TestApp {
    pub fn new(gdb_builder: Builder) -> anyhow::Result<Self> {
        // Spawn a cancellable stdin reader → broadcast::Sender
        let (stdin_tx, stdin_rx) = broadcast::channel::<String>(16);
        let stdin_task = tokio::spawn(async move {
            let mut reader = BufReader::new(tokio::io::stdin());
            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let _ = stdin_tx.send(buf.clone());
                    }
                    Err(_) => break,
                }
            }
        });

        // Build GDB + recording
        let (gdb, recording) = RecordingBuilder::new()
            .path("validation/recording-simple.json")
            .overwrite(true)
            .build(gdb_builder)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let gdb_stdout = gdb.subscribe_stdout();
        let debugger_state = CommandQueue::new(gdb.stdin_tx.clone());

        Ok(Self {
            debugger_state,
            gdb, // keep it so we can drop/shutdown later
            gdb_stdout,
            stdin_task,
            stdin_rx,
            recording,
            running: false,
        })
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        self.running = true;

        while self.running {
            self.recv_events().await;
        }

        // Graceful shutdown and persistence
        self.cleanup().await?;
        Ok(())
    }

    async fn cleanup(mut self) -> anyhow::Result<()> {
        // 1) Stop stdin task
        self.stdin_task.abort();
        let _ = self.stdin_task.await;

        // 2) Optionally send an explicit "quit" if not already sent
        // let _ = self.debugger_state.queue(&DebuggerCommand::Raw("quit".into()));

        // 3) Drop receivers then GDB handle to tear down child and background tasks
        drop(self.gdb_stdout);
        // Dropping `self.gdb` should terminate its background tasks/child process.
        drop(self.gdb);

        // 4) Persist recording atomically (suggested in earlier feedback)
        self.recording.stop()?;

        Ok(())
    }

    fn handle_gdb_output(&mut self, line: &str) {
        print!("> {line}");

        // Parse MI if possible; still record raw line even if parsing fails
        match gdb::mi::parse(line) {
            Ok(mi) => {
                println!("{}", mi.pretty());
                let id = self.recording.push_gdb_output(line.to_string(), mi.clone());

                if let Ok(evt) = self.debugger_state.lift(mi.clone()) {
                    println!("{}", evt.pretty());
                    self.recording.push_dibbuk_event(id, evt);
                }

                // Heuristic: GDB echoes `quit` to the log stream; exit loop
                if let MiRecord::LogStream(s) = mi {
                    if s.trim_end() == "quit" {
                        self.stop();
                    }
                }
            }
            Err(err) => {
                eprintln!("MI parse error: {err}");
                // If you have a sentinel MiRecord variant for unknown lines, use it.
                // Otherwise consider pushing a placeholder or skip the MI part:
                // self.recording.push_gdb_output(line.to_string(), MiRecord::Unknown(...));
            }
        }
    }

    fn handle_user_input(&mut self, line: &str) {
        let trimmed = line.trim_end().to_string();
        self.debugger_state
            .queue(&DebuggerCommand::Raw(trimmed.clone()));
        self.recording.push_user_input(line.to_string());
    }

    fn stop(&mut self) {
        self.running = false;
    }

    async fn recv_events(&mut self) {
        tokio::select! {
            line = self.gdb_stdout.recv() => {
                if let Ok(line) = line {
                    self.handle_gdb_output(&line);
                } else {
                    // Sender dropped: GDB likely exited. Stop the loop.
                    self.stop();
                }
            }
            line = self.stdin_rx.recv() => {
                if let Ok(line) = line {
                    self.handle_user_input(&line);
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                self.debugger_state.update(&DebuggerEvent::Tick);
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let gdb_builder = gdb::Builder::new().push_arg("./resources/drywall");
    let app = TestApp::new(gdb_builder)?;
    app.run().await?;
    println!("bye!");
    Ok(())
}

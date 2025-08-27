use dibbuk::{
    debugger::{
        State,
        test::{self, Recording, RecordingBuilder},
    },
    gdb::{self, Builder, mi::MiRecord, process::GdbHandle},
    il::{DebuggerCommand, DebuggerEvent},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    sync::{broadcast, oneshot},
    task::JoinHandle,
};

use facet_pretty::FacetPretty;

struct TestApp {
    debugger_state: State,
    gdb: broadcast::Receiver<String>,
    stdin_reader_handle: JoinHandle<()>,
    user: broadcast::Receiver<String>,
    recording: Recording,
    running: bool,
}

impl TestApp {
    pub fn new(gdb_builder: Builder) -> Self {
        let mut stdin_reader = BufReader::new(tokio::io::stdin());
        let (stdin_tx, stdin_rx) = broadcast::channel(16);
        let stdin_fut = tokio::spawn(async move {
            let mut line_buf = String::new();
            loop {
                line_buf.clear();
                match stdin_reader.read_line(&mut line_buf).await {
                    Ok(0) => {
                        break;
                    }
                    Ok(_) => {
                        stdin_tx.send(line_buf.clone()).unwrap();
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        let (gdb, recording) = RecordingBuilder::new()
            .path("validation/recording-simple.json")
            .overwrite(true)
            .build(gdb_builder)
            .unwrap();

        TestApp {
            recording: recording,
            debugger_state: State::new(gdb.stdin_tx.clone()),
            gdb: gdb.subscribe_stdout(),
            stdin_reader_handle: stdin_fut,
            user: stdin_rx,
            running: false,
        }
    }

    pub async fn run(mut self) {
        self.running = true;

        while self.running {
            self.recv_events().await;
        }

        println!("Stopping app...");
        self.recording.stop().unwrap();
    }

    fn handle_gdb_output(&mut self, line: &str) {
        print!("> {line}");
        let mi = gdb::mi::parse(line).unwrap();
        println!("{}", mi.pretty());
        let mi_rec_id = self.recording.push_gdb_output(line.to_string(), mi.clone());

        let evt = self.debugger_state.lift(mi.clone()).unwrap();
        println!("{}", evt.pretty());

        self.recording.push_dibbuk_event(mi_rec_id, evt);

        if let MiRecord::LogStream(s) = &mi {
            if s == "quit\n" {
                self.stop();
            }
        }
    }

    fn handle_user_input(&mut self, line: &str) {
        self.debugger_state
            .queue(&DebuggerCommand::Raw(line.trim_end().to_string()));
        self.recording.push_user_input(line.to_string());
    }

    fn stop(&mut self) {
        self.running = false;
    }

    async fn recv_events(&mut self) {
        tokio::select! {
            line = self.gdb.recv() => { if let Ok(line) = line { self.handle_gdb_output(line.as_str()); } }
            line = self.user.recv() => { if let Ok(line) = line { self.handle_user_input(line.as_str()); } }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => { self.debugger_state.update(&DebuggerEvent::Tick); }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("hello!");

    let gdb_builder = gdb::Builder::new().push_arg("./resources/drywall");
    let app = TestApp::new(gdb_builder);
    app.run().await;
    println!("bye!");
    // FIXME: THIS DOES NOT EXIT AFTER THE BYE
}

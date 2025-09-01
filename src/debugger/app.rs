use std::collections::HashMap;

use crate::debugger::runtime::{RuntimeRequest, ScriptingRuntime};
use crate::debugger::test::RecordingBuilder;
use crate::gdb::mi::MiRecord;
use crate::gdb::process::GdbHandle;
use crate::il::DebuggerCommand;
use crate::{gdb, il};
use steel::SteelVal;
use steel::rvals::SteelHashMap;
use steel::steel_vm::{engine::Engine, register_fn::RegisterFn};
use steel_derive::Steel;
use steel_repl;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, mpsc};

use notify::{Event, RecursiveMode, Result, Watcher};
use tokio::task::JoinHandle;

pub enum DibbukError {
    FileWatch,
}

pub struct App {
    runtime: ScriptingRuntime,
    gdb_stdout: broadcast::Receiver<String>,
    running: bool,
    gdb_handle: GdbHandle,
    user_terminal_stdin: broadcast::Receiver<String>,
    runtime_fs_events_rx: mpsc::Receiver<Event>,
}

fn get_mi_events() -> SteelVal {
    let rec = RecordingBuilder::new()
        .path("./validation/mi-small.json")
        .load()
        .unwrap();

    let mi_vals: Vec<SteelVal> = rec
        .into_mi_test_cases()
        .into_iter()
        .map(|(_, mi)| mi.into())
        .collect();

    SteelVal::ListV(mi_vals.into())
}

fn spawn_user_terminal_stdin_task(
    user_terminal_stdin_tx: broadcast::Sender<String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(tokio::io::stdin());
        let mut buf = String::new();
        loop {
            buf.clear();
            match reader.read_line(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let _ = user_terminal_stdin_tx.send(buf.clone());
                }
                Err(_) => break,
            }
        }
    })
}

impl App {
    pub fn new(gdb_handle: GdbHandle) -> Self {
        let (fs_changes_tx, fs_changes_rx) = mpsc::channel::<Event>(16);

        let (user_terminal_stdin_tx, user_terminal_stdin_rx) = broadcast::channel::<String>(16);
        let stdin_task = spawn_user_terminal_stdin_task(user_terminal_stdin_tx);
        let gdb_stdout_rx = gdb_handle.subscribe_stdout();

        let runtime = crate::debugger::runtime::Builder::new().build();

        let mut watcher = notify::RecommendedWatcher::new(
            move |result: Result<Event>| match result {
                Ok(event) => {
                    println!("Sending fs update event! {event:?}");
                    if let Err(e) = fs_changes_tx.blocking_send(event) {
                        eprintln!("Failed to send event: {}", e);
                    }
                }
                Err(e) => eprintln!("Watch error: {:?}", e),
            },
            notify::Config::default(),
        )
        .unwrap();

        watcher
            .watch(
                &runtime
                    .dibbuk_main_path
                    .as_path()
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("runtime"),
                RecursiveMode::Recursive,
            )
            .unwrap();

        App {
            gdb_handle,
            runtime,
            running: true,
            gdb_stdout: gdb_stdout_rx,
            user_terminal_stdin: user_terminal_stdin_rx,
            runtime_fs_events_rx: fs_changes_rx,
        }
    }

    pub async fn run(mut self) {
        while self.running {
            self.poll_events().await;

            // let val = self.runtime.call_function_with_args(function, arguments);
        }
    }

    fn handle_user_terminal_stdin(&mut self, line: String) {
        // Using legacy IL, maybe abandon this later to raw strings
        let cmd = DebuggerCommand::UserInput(line.trim_end().to_string());
        self.runtime.event_callback(cmd.into());
        self.dispatch_gdb_commands();
    }

    fn handle_gdb_stdout(&mut self, line: String) {
        if let Ok(mi) = gdb::mi::parse(line.as_str()) {
            // TODO: the ideal version would be to return a SteelVal and pass it to the function below
            self.runtime.event_callback(mi.into());
            self.dispatch_gdb_commands();
        }
    }

    fn handle_runtime_fs_event(&mut self, evt: Event) {
        println!("received!!!! {evt:?}");
    }

    fn dispatch_gdb_commands(&mut self) {
        let cmds = self.runtime.extract_gdb_commands();
        for cmd in cmds {
            self.gdb_handle.send(cmd).unwrap();
        }
    }

    async fn poll_events(&mut self) {
        tokio::select! {
            Some(evt) = self.runtime_fs_events_rx.recv() => self.handle_runtime_fs_event(evt),
            Ok(line) = self.gdb_stdout.recv() => self.handle_gdb_stdout(line),
            Ok(line) = self.user_terminal_stdin.recv() => self.handle_user_terminal_stdin(line),
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(20)) => {
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{debugger::app::App, gdb};

    #[tokio::test]
    async fn simple_repl() {
        let gdb_handle = gdb::Builder::new()
            .push_arg("./resources/frog")
            .spawn()
            .unwrap();
        App::new(gdb_handle).run().await;
    }
}

// use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
// use std::path::Path;
// use tokio::sync::mpsc;
// use tokio::time::{sleep, Duration};

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
// // Create a channel to receive file system events
// let (tx, mut rx) = mpsc::channel(100);

// ```
// // Create the watcher
// let mut watcher = RecommendedWatcher::new(
//     move |result: Result<Event, notify::Error>| {
//         match result {
//             Ok(event) => {
//                 // Send the event through the channel
//                 if let Err(e) = tx.blocking_send(event) {
//                     eprintln!("Failed to send event: {}", e);
//                 }
//             }
//             Err(e) => eprintln!("Watch error: {:?}", e),
//         }
//     },
//     Config::default(),
// )?;

// // Start watching a directory (change this path as needed)
// let watch_path = "./watched_directory";
// watcher.watch(Path::new(watch_path), RecursiveMode::Recursive)?;

// println!("Watching directory: {}", watch_path);
// println!("Press Ctrl+C to stop...");

// // Process events asynchronously
// while let Some(event) = rx.recv().await {
//     handle_file_event(event).await;
// }

// Ok(())
// ```

// }

// async fn handle_file_event(event: Event) {
// match event.kind {
// notify::EventKind::Create(*) => {
// println!(“📁 Created: {:?}”, event.paths);
// // Example: Process newly created files
// for path in &event.paths {
// if path.is_file() {
// process_new_file(path).await;
// }
// }
// }
// notify::EventKind::Modify(*) => {
// println!(“✏️  Modified: {:?}”, event.paths);
// // Example: React to file modifications
// for path in &event.paths {
// if path.is_file() {
// process_modified_file(path).await;
// }
// }
// }
// notify::EventKind::Remove(*) => {
// println!(“🗑️  Removed: {:?}”, event.paths);
// // Example: Clean up resources for deleted files
// for path in &event.paths {
// process_deleted_file(path).await;
// }
// }
// notify::EventKind::Rename(*) => {
// println!(“📝 Renamed: {:?}”, event.paths);
// }
// _ => {
// println!(“🔍 Other event: {:?} - {:?}”, event.kind, event.paths);
// }
// }
// }

// async fn process_new_file(path: &Path) {
// println!(“Processing new file: {:?}”, path);
// // Simulate some async work
// sleep(Duration::from_millis(100)).await;

// ```
// // Example: Read file content
// if let Ok(content) = tokio::fs::read_to_string(path).await {
//     println!("File content length: {} bytes", content.len());
// }
// ```

// }

// async fn process_modified_file(path: &Path) {
// println!(“Processing modified file: {:?}”, path);
// // Simulate some async work
// sleep(Duration::from_millis(50)).await;

// ```
// // Example: Get file metadata
// if let Ok(metadata) = tokio::fs::metadata(path).await {
//     println!("File size: {} bytes", metadata.len());
// }
// ```

// }

// async fn process_deleted_file(path: &Path) {
// println!(“Cleaning up resources for deleted file: {:?}”, path);
// // Simulate cleanup work
// sleep(Duration::from_millis(25)).await;

// ```
// // Example: Remove from cache, close handles, etc.
// // This is where you'd clean up any resources associated with the file
// ```

// }

// // Alternative version using a more structured approach with a FileWatcher struct
// pub struct FileWatcher {
// _watcher: RecommendedWatcher,
// receiver: mpsc::Receiver<Event>,
// }

// impl FileWatcher {
// pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
// let (tx, rx) = mpsc::channel(100);

// ```
//     let mut watcher = RecommendedWatcher::new(
//         move |result: Result<Event, notify::Error>| {
//             if let Ok(event) = result {
//                 let _ = tx.blocking_send(event);
//             }
//         },
//         Config::default(),
//     )?;

//     watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

//     Ok(FileWatcher {
//         _watcher: watcher,
//         receiver: rx,
//     })
// }

// pub async fn next_event(&mut self) -> Option<Event> {
//     self.receiver.recv().await
// }
// ```

// }

// // Example usage of the structured approach
// async fn _example_structured_usage() -> Result<(), Box<dyn std::error::Error>> {
// let mut watcher = FileWatcher::new(”./watched_directory”)?;

// ```
// while let Some(event) = watcher.next_event().await {
//     println!("Received event: {:?}", event);
//     // Process event here
// }

// Ok(())
// ```

// }

use dibbuk::{debugger::app::App, *};

#[tokio::main]
async fn main() {
    let gdb_handle = gdb::Builder::new()
        .push_arg("./resources/frog")
        .spawn()
        .unwrap();
    App::new(gdb_handle).run().await;
}

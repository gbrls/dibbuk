mod elf;
mod process;
mod parser;
mod mi_types;


#[tokio::main]
async fn main() {
    process::start().await;
}

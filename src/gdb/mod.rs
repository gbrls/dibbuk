use tokio::sync::{broadcast, mpsc};

use crate::IOTask;

pub mod lift_mi;
pub mod mi;
pub mod process;

pub use process::Builder;

impl IOTask for Builder {
    fn start(
        self,
        rx: mpsc::UnboundedReceiver<String>,
        tx: broadcast::Sender<String>,
    ) -> tokio::task::JoinHandle<()> {
        // tokio::spawn(self::process::run_event_loop(rx, tx))
        todo!()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // TODO: integration tests
}

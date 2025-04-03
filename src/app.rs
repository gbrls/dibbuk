use std::collections::HashMap;

use crate::gdb;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GdbState {
    Unknown,
    Running,
    Stopped,
}

#[derive(Debug)]
pub struct GdbContext {
    pub register_name: HashMap<usize, String>,
    pub register_id: HashMap<String, usize>,
    pub register_value: HashMap<usize, Option<u64>>,
    pub state: GdbState,
}

#[derive(Debug)]
pub struct DibbukState {
    pub gdb_ctx: GdbContext,
}

impl DibbukState {
    pub fn new() -> Self {
        Self {
            gdb_ctx: GdbContext {
                register_name: HashMap::new(),
                register_id: HashMap::new(),
                register_value: HashMap::new(),
                state: GdbState::Unknown,
            },
        }
    }

    pub async fn run(
        mut state: std::sync::Arc<tokio::sync::RwLock<DibbukState>>,
        mut gdb_rx: Receiver<gdb::OutputEvent>,
        gdb_tx: UnboundedSender<gdb::GdbCommand>,
    ) {
        loop {
            while let Ok(cmd) = gdb_rx.recv().await {
                use crate::parser::AsyncKind;
                use crate::parser::AsyncRecord;
                use crate::parser::MiRecord;
                use crate::parser::MiValue;
                use crate::parser::ResultRecord;
                match cmd.mi {
                    None => {}
                    Some(MiRecord::Result(ResultRecord { results, .. }))
                        if results.contains_key("register-names") =>
                    {
                        if let MiValue::List(regs) = results.get("register-names").unwrap() {
                            let mut state = state.write().await;
                            for (i, r) in regs.iter().enumerate() {
                                if let MiValue::Const(s) = r {
                                    state.gdb_ctx.register_name.insert(i, s.clone());
                                    state.gdb_ctx.register_id.insert(s.clone(), i);
                                }
                            }
                        }
                    }
                    Some(MiRecord::Result(ResultRecord { results, .. }))
                        if results.contains_key("register-values") => {}
                    Some(MiRecord::ExecAsync(AsyncRecord {
                        kind: AsyncKind::Exec,
                        class,
                        ..
                    })) => match class.as_str() {
                        "stopped" => {
                            let mut state = state.write().await;
                            state.gdb_ctx.state = GdbState::Stopped
                        }
                        "running" => {
                            let mut state = state.write().await;
                            state.gdb_ctx.state = GdbState::Running
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }
}

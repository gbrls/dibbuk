use std::collections::HashMap;

use crate::process;
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
    pub register_value: HashMap<String, u64>,
    pub state: GdbState,
}

impl GdbContext {
    pub fn new() -> Self {
        Self {
            register_name: HashMap::new(),
            register_id: HashMap::new(),
            register_value: HashMap::new(),
            state: GdbState::Unknown,
        }
    }
}

pub async fn run(mut data: crate::AppDataHandle) {
    loop {
        while let Ok(cmd) = data.channels.gdb_mi_rx.recv().await {
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
                        let mut state = data.state.write().await;
                        for (i, r) in regs.iter().enumerate() {
                            if let MiValue::Const(s) = r {
                                state.gdb_ctx.register_name.insert(i, s.clone());
                                state.gdb_ctx.register_id.insert(s.clone(), i);
                            }
                        }
                    }
                }
                Some(MiRecord::Result(ResultRecord { results, .. }))
                    if results.contains_key("register-values") =>
                {
                    println!("register values!");
                    //Tuple([("number", Const("0")), ("value", Const("0x5555555555d0"))]),
                    if let MiValue::List(tuple_list) = results.get("register-values").unwrap() {
                        for tpl in tuple_list {
                            if let MiValue::Tuple(tpl) = tpl {
                                let mut idx: Option<u64> = None;
                                let mut value: Option<u64> = None;
                                for (k, v) in tpl {
                                    match (k.as_str(), v) {
                                        ("number", MiValue::Const(s)) => {
                                            idx = u64::from_str_radix(s.as_str(), 10).ok();
                                        }
                                        ("value", MiValue::Const(s)) => {
                                            value = u64::from_str_radix(
                                                s.as_str().trim_start_matches("0x"),
                                                16,
                                            )
                                            .ok();
                                        }
                                        _ => {}
                                    }
                                }

                                let reg_name = {
                                    let state = data.state.read().await;
                                    if idx.is_some() {
                                        state
                                            .gdb_ctx
                                            .register_name
                                            .get(&(idx.unwrap() as usize))
                                            .cloned()
                                    } else {
                                        None
                                    }
                                };
                                match (reg_name, value) {
                                    (Some(k), Some(v)) => {
                                        let mut state = data.state.write().await;
                                        state.gdb_ctx.register_value.insert(k.clone(), v);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Some(MiRecord::ExecAsync(AsyncRecord {
                    kind: AsyncKind::Exec,
                    class,
                    ..
                })) => match class.as_str() {
                    "stopped" => {
                        let mut state = data.state.write().await;
                        state.gdb_ctx.state = GdbState::Stopped
                    }
                    "running" => {
                        let mut state = data.state.write().await;
                        state.gdb_ctx.state = GdbState::Running
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

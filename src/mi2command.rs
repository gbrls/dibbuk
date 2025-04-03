use std::collections::HashMap;

use crate::process;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GdbState {
    Unknown,
    Running,
    Stopped,
    Exited,
}

#[derive(Debug, Clone)]
pub enum GdbMessage {
    RegisterValue(Vec<(String, u64)>),
    UpdatedRegisters(Vec<usize>),
    StateUpdate(GdbState),
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
            use crate::AppEvent::Gdb;
            use GdbMessage::*;

            // [mi] ExecAsync(AsyncRecord { token: None, kind: Exec, class: "stopped", results: {"frame": Tuple([("addr", Const("0x00005555555555d5")), ("func", Const("main")), ("args", List([])), ("arch", Const("i386:x86-64"))]), "thread-id": Const("1"), "reason": Const("end-stepping-range"), "core": Const("12"), "stopped-threads": Const("all")} })
            //
            // [mi] ExecAsync(AsyncRecord { token: None, kind: Exec, class: "stopped", results: {"thread-id": Const("1"), "stopped-threads": Const("all"), "frame": Tuple([("addr", Const("0x00005555555555d3")), ("func", Const("main")), ("args", List([])), ("arch", Const("i386:x86-64"))]), "core": Const("12"), "reason": Const("end-stepping-range")} })

            match cmd.mi {
                None => {}
                Some(MiRecord::ExecAsync(AsyncRecord {
                    kind: AsyncKind::Exec,
                    class,
                    results,
                    ..
                })) => {
                    let reason = results.get("reason");
                    let update = match (class.as_str(), reason) {
                        ("stopped", Some(MiValue::Const(r))) if r.as_str() == "exited" => {
                            let updated = GdbState::Exited;
                            let mut state = data.state.write().await;
                            state.gdb_ctx.state = updated;
                            Some(updated)
                        }

                        ("stopped", _) => {
                            let updated = GdbState::Stopped;
                            let mut state = data.state.write().await;
                            state.gdb_ctx.state = updated;
                            Some(updated)
                        }
                        ("running", _) => {
                            let updated = GdbState::Running;
                            let mut state = data.state.write().await;
                            state.gdb_ctx.state = updated;
                            Some(updated)
                        }
                        (_, _) => None,
                    };

                    if update.is_some() {
                        data.channels
                            .event_tx
                            .send(Gdb(StateUpdate(update.unwrap())))
                            .unwrap();
                    }
                }

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

                // Result(ResultRecord { token: None, class: "done", results: {"changed-registers": List([Const("0"), Const("1"), Const("2"), Const("3"),
                Some(MiRecord::Result(ResultRecord { results, .. }))
                    if results.contains_key("changed-registers") =>
                {
                    if let MiValue::List(ids) = results.get("changed-registers").unwrap() {
                        let v = ids
                            .iter()
                            .map(|i| match i {
                                MiValue::Const(s) => usize::from_str_radix(s, 10).unwrap(),
                                _ => panic!("unkown reg value {:?}", i),
                            })
                            .collect();
                        data.channels
                            .event_tx
                            .send(Gdb(UpdatedRegisters(v)))
                            .unwrap();
                    }
                }

                Some(MiRecord::Result(ResultRecord { results, .. }))
                    if results.contains_key("register-values") =>
                {
                    //Tuple([("number", Const("0")), ("value", Const("0x5555555555d0"))]),
                    if let MiValue::List(tuple_list) = results.get("register-values").unwrap() {
                        let mut register_values = Vec::new();
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
                                        register_values.push((k.clone(), v));
                                    }
                                    _ => {}
                                }
                            }
                        }

                        data.channels
                            .event_tx
                            .send(Gdb(RegisterValue(register_values)))
                            .unwrap();
                    }
                }
                _ => {}
            }
        }
    }
}

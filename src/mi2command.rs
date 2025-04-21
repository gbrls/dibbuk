use std::collections::HashMap;

use crate::process;
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GdbState {
    Unknown,
    Running,
    Stopped,
    Exited,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Disassembly {
    pub str: String,
    pub func: String,
    pub offset: usize,
    pub addr: usize,
}

#[derive(Debug, Clone)]
pub struct MemMap {
    pub map_range: proc_maps::MapRange,
}

impl PartialEq for MemMap {
    fn eq(&self, other: &Self) -> bool {
        (self.map_range.start() == other.map_range.start())
            && (self.map_range.inode == other.map_range.inode)
    }
}

impl Eq for MemMap {
    fn assert_receiver_is_total_eq(&self) {}
}

impl PartialOrd for MemMap {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.map_range.start().cmp(&other.map_range.start()))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub struct StackFrame {
    pub depth: u64,
    pub addr: u64,
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq)]
pub enum GdbMessage {
    Pid(u64),
    Maps(Vec<MemMap>),
    RegisterValue(Vec<(String, u64)>),
    UpdatedRegisters(Vec<usize>),
    StateUpdate(GdbState),
    DisassemblyNative(Vec<Disassembly>),
    StackFrames(Vec<StackFrame>),
}

//TODO: refactor with generics to handle other architectures
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

    //TODO: refactor with generics to handle other architectures
    pub fn instruction_pointer_value(&self) -> Option<u64> {
        self.register_value.get("rip".into()).cloned()
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
            use crate::AppEvent::*;
            use GdbMessage::*;

            // [mi] ExecAsync(AsyncRecord { token: None, kind: Exec, class: "stopped", results: {"frame": Tuple([("addr", Const("0x00005555555555d5")), ("func", Const("main")), ("args", List([])), ("arch", Const("i386:x86-64"))]), "thread-id": Const("1"), "reason": Const("end-stepping-range"), "core": Const("12"), "stopped-threads": Const("all")} })
            //
            // [mi] ExecAsync(AsyncRecord { token: None, kind: Exec, class: "stopped", results: {"thread-id": Const("1"), "stopped-threads": Const("all"), "frame": Tuple([("addr", Const("0x00005555555555d3")), ("func", Const("main")), ("args", List([])), ("arch", Const("i386:x86-64"))]), "core": Const("12"), "reason": Const("end-stepping-range")} })

            // always send raw mi commands first
            //if cmd.mi.is_some() {
            //    data.channels
            //        .event_tx
            //        .send(GdbMi(cmd.mi.clone().unwrap()))
            //        .unwrap();
            //}

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
                        //println!("{:?}", regs);
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
                    if results.contains_key("asm_insns") =>
                {
                    // [mi] Result(ResultRecord { token: None, class: "done", results: {"asm_insns": List([Tuple([("address", Const("0x00005555555555d5")), ("func-name", Const("main")), ("offset", Const("5")), ("inst", Const("push   %r13"))])
                    if let MiValue::List(asm_tuples) = results.get("asm_insns").unwrap() {
                        let mut asm_lines = Vec::new();
                        for mival_asmtuples in asm_tuples {
                            let mut addr = None;
                            let mut offset = None;
                            let mut fname = None;
                            let mut inst = None;
                            if let MiValue::Tuple(asm_tuple) = mival_asmtuples {
                                for (k, v) in asm_tuple {
                                    match (k.as_str(), v) {
                                        ("address", MiValue::Const(v)) => {
                                            addr = usize::from_str_radix(
                                                v.as_str().strip_prefix("0x").unwrap_or(""),
                                                16,
                                            )
                                            .ok()
                                        }

                                        ("offset", MiValue::Const(v)) => {
                                            offset = usize::from_str_radix(v, 10).ok()
                                        }
                                        ("func-name", MiValue::Const(v)) => fname = Some(v),
                                        ("inst", MiValue::Const(v)) => inst = Some(v),
                                        _ => {}
                                    }
                                }
                            }

                            match (addr, offset, fname, inst) {
                                (Some(addr), Some(offset), Some(fname), Some(instr)) => asm_lines
                                    .push(Disassembly {
                                        offset,
                                        addr,
                                        str: instr.clone(),
                                        func: fname.clone(),
                                    }),
                                _ => {}
                            }
                        }

                        if asm_lines.len() > 0 {
                            data.channels
                                .event_tx
                                .send(Gdb(DisassemblyNative(asm_lines)))
                                .unwrap();
                        }
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

                Some(MiRecord::Result(ResultRecord { results, .. }))
                    if results.contains_key("threads") =>
                {
                    let threads = results.get("threads").unwrap();
                    if let MiValue::List(tuple_list) = threads {
                        for tpl in tuple_list {
                            if let MiValue::Tuple(tpl) = tpl {
                                for (k, v) in tpl {
                                    match (k, v) {
                                        (_, MiValue::Const(v)) if k == "target-id" => {
                                            let re = regex::Regex::new(r".* (\d+)")
                                                .expect("Invalid Regex pattern");
                                            if let Some(caps) = re.captures(v) {
                                                let pid_str = caps.iter().last();
                                                if let Some(Some(pid_str)) = pid_str {
                                                    let pid = pid_str.as_str().parse::<u64>();
                                                    if let Ok(pid) = pid {
                                                        data.channels
                                                            .event_tx
                                                            .send(Gdb(Pid(pid)))
                                                            .unwrap();
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }

                Some(MiRecord::Result(ResultRecord { results, .. }))
                    if results.contains_key("stack") =>
                {
                    let stack = results.get("stack").unwrap();
                    if let MiValue::List(tuple_list) = stack {
                        let mut frames = Vec::new();
                        for tpl in tuple_list {
                            if let MiValue::Tuple(tpl) = tpl {
                                for (k, v) in tpl {
                                    match (k, v) {
                                        (v, MiValue::Tuple(frame_info)) => {
                                            let mut line = None;
                                            let mut file = None;
                                            let mut addr = None;
                                            let mut function = None;
                                            let mut depth = None;
                                            for (k, v) in frame_info {
                                                match v {
                                                    MiValue::Const(level) if k == "level" => {
                                                        depth = level.as_str().parse::<u64>().ok();
                                                    }
                                                    MiValue::Const(adr) if k == "addr" => {
                                                        addr = u64::from_str_radix(
                                                            adr.as_str()
                                                                .strip_prefix("0x")
                                                                .unwrap_or(""),
                                                            16,
                                                        )
                                                        .ok();
                                                    }
                                                    MiValue::Const(f) if k == "func" => {
                                                        function = Some(f);
                                                    }
                                                    MiValue::Const(f) if k == "fullname" => {
                                                        file = Some(f);
                                                    }
                                                    MiValue::Const(l) if k == "line" => {
                                                        line = l.as_str().parse::<u64>().ok();
                                                    }
                                                    _ => {}
                                                }
                                            }

                                            match (depth, addr) {
                                                (Some(depth), Some(addr)) => {
                                                    let frame = StackFrame {
                                                        addr,
                                                        depth,
                                                        line,
                                                        function: function.cloned(),
                                                        file: file.cloned(),
                                                    };
                                                    frames.push(frame);
                                                }
                                                _ => {}
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }

                        if !frames.is_empty() {
                            data.channels
                                .event_tx
                                .send(crate::AppEvent::Gdb(GdbMessage::StackFrames(frames)))
                                .unwrap();
                        }
                    }
                }
                Some(mi) => {
                    data.channels.event_tx.send(GdbMi(mi)).unwrap();
                }

                _ => {}
            }
        }
    }
}

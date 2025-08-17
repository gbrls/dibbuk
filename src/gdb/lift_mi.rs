use crate::gdb::parser::{MiRecord, ResultRecord};
use crate::il;
use crate::process::MiOutput;
use std::collections::HashMap;

//TODO: refactor with generics to handle other architectures
#[derive(Debug)]
pub struct GdbLifterContext {
    pub register_name: HashMap<usize, String>,
    pub register_id: HashMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiftError {
    UnknownOpcode(u8),
    ExpectedList(String),
    ExpectedString(String),
    InvalidRegisterId(String),
    Multiple(Vec<LiftError>),
}

impl GdbLifterContext {
    pub fn new() -> Self {
        Self {
            register_name: HashMap::new(),
            register_id: HashMap::new(),
        }
    }

    pub fn lift_asm_insns(result: &ResultRecord) -> Result<Option<il::Message>, LiftError> {
        // [mi] Result(ResultRecord { token: None, class: "done", results: {"asm_insns": List([Tuple([("address", Const("0x00005555555555d5")), ("func-name", Const("main")), ("offset", Const("5")), ("inst", Const("push   %r13"))])
        use super::parser::*;
        use il::*;
        if let MiValue::List(asm_tuples) = result.results.get("asm_insns").unwrap() {
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
                    (Some(addr), Some(offset), Some(fname), Some(instr)) => {
                        asm_lines.push(il::Disassembly {
                            source: il::DisassemblyEngine::GDB,
                            offset,
                            addr,
                            str: instr.clone(),
                            func: fname.clone(),
                            operand: None,
                            mnemonic: None,
                        })
                    }
                    _ => {}
                }
            }

            Ok(Some(Message::Disassembly(asm_lines)))
        } else {
            Err(LiftError::ExpectedList(String::from("asm_insns")))
        }
    }
    pub fn lift_threads(result: &ResultRecord) -> Result<Option<il::Message>, LiftError> {
        use super::parser::*;
        use il::*;
        if let MiValue::List(tuple_list) = result.results.get("threads").unwrap() {
            for tpl in tuple_list {
                if let MiValue::Tuple(tpl) = tpl {
                    for (k, v) in tpl {
                        match (k, v) {
                            (_, MiValue::Const(v)) if k == "target-id" => {
                                let re =
                                    regex::Regex::new(r".* (\d+)").expect("Invalid Regex pattern");
                                if let Some(caps) = re.captures(v)
                                    && let Some(Some(pid_str)) = caps.iter().last()
                                    && let Ok(pid) = pid_str.as_str().parse::<u64>()
                                {
                                    // TODO: is this sound? is it safe to always return the first target-id that is found?
                                    return Ok(Some(Message::Pid(pid)));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(None)
        } else {
            Err(LiftError::ExpectedList(String::from("threads")))
        }
    }

    pub fn lift_stack(result: &ResultRecord) -> Result<Option<il::Message>, LiftError> {
        use super::parser::*;
        use il::*;
        if let MiValue::List(tuple_list) = result.results.get("stack").unwrap() {
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
                                                adr.as_str().strip_prefix("0x").unwrap_or(""),
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
                                        let frame = il::StackFrame {
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

            Ok(Some(il::Message::StackFrames(frames)))
        } else {
            Err(LiftError::ExpectedList(String::from("stack")))
        }
    }
    pub fn update_register_names(&mut self, result: &ResultRecord) -> Result<(), LiftError> {
        use super::parser::*;
        if let MiValue::List(regs) = result.results.get("register-names").unwrap() {
            for (i, r) in regs.iter().enumerate() {
                if let MiValue::Const(s) = r {
                    self.register_name.insert(i, s.clone());
                    self.register_id.insert(s.clone(), i);
                }
            }
        }
        Ok(())
    }

    pub fn lift_changed_registers(
        &self,
        result: &ResultRecord,
    ) -> Result<Option<il::Message>, LiftError> {
        use super::parser::*;
        use il::*;

        // Result(ResultRecord { token: None, class: "done", results: {"changed-registers": List([Const("0"), Const("1"), Const("2"), Const("3"),
        if let MiValue::List(ids) = result.results.get("changed-registers").unwrap() {
            let register_ids: Vec<_> = ids
                .iter()
                .map(|i| match i {
                    MiValue::Const(s) => usize::from_str_radix(s, 10)
                        .map_err(|_| LiftError::InvalidRegisterId(s.clone())),
                    _ => Err(LiftError::ExpectedString("register id".into())),
                })
                .collect();

            let errs: Vec<_> = register_ids
                .iter()
                .filter_map(|r| match r {
                    Ok(_) => None,
                    Err(e) => Some(e.clone()),
                })
                .collect();

            if !errs.is_empty() {
                Err(LiftError::Multiple(errs))
            } else {
                // TODO: this fails silently in case of register ids that aren't mapped
                Ok(Some(Message::UpdatedRegisters(
                    register_ids
                        .iter()
                        .filter_map(|id| self.register_name.get(id.as_ref().unwrap()))
                        .cloned()
                        .collect(),
                )))
            }
        } else {
            Err(LiftError::ExpectedList("changed-registers".into()))
        }
    }

    pub fn lift_register_values(
        &self,
        result: &ResultRecord,
    ) -> Result<Option<il::Message>, LiftError> {
        use super::parser::*;
        use il::*;

        if let MiValue::List(tuple_list) = result.results.get("register-values").unwrap() {
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
                                value =
                                    u64::from_str_radix(s.as_str().trim_start_matches("0x"), 16)
                                        .ok();
                            }
                            _ => {}
                        }
                    }

                    let reg_name = {
                        if idx.is_some() {
                            self.register_name.get(&(idx.unwrap() as usize)).cloned()
                        } else {
                            None
                        }
                    };
                    match (reg_name, value) {
                        (Some(k), Some(v)) => {
                            register_values.push((k.clone(), v));
                        }
                        _ => {}
                    }
                }
            }

            Ok(Some(Message::RegisterValue(register_values)))
        } else {
            Err(LiftError::ExpectedList("register-values".into()))
        }
    }

    pub fn lift(&mut self, value: MiRecord) -> Result<Option<il::Message>, LiftError> {
        use super::parser::*;
        use il::*;

        match &value {
            MiRecord::ExecAsync(AsyncRecord {
                kind: AsyncKind::Exec,
                class,
                results,
                ..
            }) => {
                let reason = results.get("reason");
                use ExecutionState::*;
                match (class.as_str(), reason) {
                    ("stopped", Some(MiValue::Const(reason))) if reason.as_str() == "exited" => {
                        Ok(Some(Message::StateUpdate(Exited)))
                    }

                    ("stopped", _) => Ok(Some(Message::StateUpdate(Stopped))),
                    ("running", _) => Ok(Some(Message::StateUpdate(Running))),
                    (_, _) => Ok(None),
                }
            }

            MiRecord::Result(result @ ResultRecord { results, .. })
                if results.contains_key("asm_insns") =>
            {
                GdbLifterContext::lift_asm_insns(&result)
            }

            MiRecord::Result(result @ ResultRecord { results, .. })
                if results.contains_key("threads") =>
            {
                GdbLifterContext::lift_threads(&result)
            }

            // ││[12] GdbMi(Result(ResultRecord { token: None, class: "done", results: {"cwd": Const("/home/gbrls/Documents/vr-src/v8")} }))
            MiRecord::Result(ResultRecord { results, .. }) if results.contains_key("cwd") => {
                if let MiValue::Const(cwd) = results.get("cwd").unwrap() {
                    Ok(Some(il::Message::Cwd(cwd.clone())))
                } else {
                    Err(LiftError::ExpectedString(String::from("cwd")))
                }
            }

            MiRecord::Result(result @ ResultRecord { results, .. })
                if results.contains_key("stack") =>
            {
                GdbLifterContext::lift_stack(&result)
            }

            MiRecord::Result(result @ ResultRecord { results, .. })
                if results.contains_key("register-names") =>
            {
                // TODO: maybe send just the register names from IL?
                let _ = self.update_register_names(result);
                Ok(None)
            }

            MiRecord::Result(result @ ResultRecord { results, .. })
                if results.contains_key("changed-registers") =>
            {
                self.lift_changed_registers(result)
            }

            MiRecord::Result(result @ ResultRecord { results, .. })
                if results.contains_key("register-values") =>
            {
                //Tuple([("number", Const("0")), ("value", Const("0x5555555555d0"))]),
                self.lift_register_values(result)
            }
            _ => Ok(None),
        }
    }
}

pub async fn run(mut data: crate::AppDataHandle) {
    loop {
        while let Ok(cmd) = data.channels.gdb_mi_rx.recv().await {
            // always send raw mi commands first
            //if cmd.mi.is_some() {
            //    data.channels
            //        .event_tx
            //        .send(GdbMi(cmd.mi.clone().unwrap()))
            //        .unwrap();
            //}

            match cmd.mi {
                None => {}

                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lift_gdb_exec_async() {
        use crate::gdb::parser::*;
        use MiRecord::*;

        let mut lifter = GdbLifterContext::new();
        assert_eq!(
            lifter.lift(ExecAsync(AsyncRecord {
                token: None,
                kind: AsyncKind::Exec,
                class: "stopped".into(),
                results: HashMap::new(),
            })),
            Ok(Some(il::Message::StateUpdate(il::ExecutionState::Stopped)))
        );
    }
}

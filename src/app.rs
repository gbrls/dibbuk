use std::collections::HashMap;

pub struct GdbContext {
    pub register_name: HashMap<usize, String>,
    pub register_id: HashMap<String, usize>,
    pub register_value: HashMap<usize, Option<u64>>,
}

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
            },
        }
    }
}

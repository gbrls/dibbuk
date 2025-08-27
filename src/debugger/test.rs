use anyhow::Result;
use facet::Facet;
use facet_json::{DeserError, from_str, to_string};

use crate::gdb::mi::MiRecord;

#[derive(Debug, Clone, Facet)]
pub struct Recording {
    path: String,
    name: String,
    ids: u64,
    gdb_output: Vec<(u64, String, MiRecord)>,
    user_input: Vec<(u64, String)>,
}

impl Recording {
    pub fn push_gdb_output(&mut self, out: String, mi: MiRecord) {
        // self.gdb_output.push((Instant::now(), out, mi));
        self.gdb_output.push((self.ids, out, mi));
        self.ids += 1;
    }

    pub fn push_user_input(&mut self, input: String) {
        // self.user_input.push((Instant::now(), input));
        self.user_input.push((self.ids, input));
        self.ids += 1;
    }

    pub fn stop(self) -> Result<(), std::io::Error> {
        let r = self::to_string(&self);

        std::fs::write(self.path, r)
    }

    fn load(input: String) -> Self {
        self::from_str(input.as_str()).unwrap()
    }

    // pub fn store()
}

pub struct RecordingBuilder {
    path: String,
    name: String,
}

impl RecordingBuilder {
    pub fn new() -> Self {
        RecordingBuilder {
            path: String::new(),
            name: String::new(),
        }
    }

    pub fn path(mut self, p: impl Into<String>) -> Self {
        self.path = p.into();
        self
    }

    pub fn load(self) -> Result<Recording> {
        let str = std::fs::read_to_string(self.path)?;
        let rec = Recording::load(str);
        Ok(rec)
    }

    pub fn build(self) -> Recording {
        Recording {
            // start: Instant::now(),
            ids: 0,
            path: self.path,
            name: self.name,
            gdb_output: Vec::new(),
            user_input: Vec::new(),
            // finish: None,
        }
    }
}

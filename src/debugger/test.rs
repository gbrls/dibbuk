use facet::Facet;
use facet_json::{from_str, to_string};
use thiserror::Error;

use crate::{
    gdb::{self, mi::MiRecord, process::GdbHandle},
    il::{DebuggerCommand, DebuggerEvent},
};

#[derive(Debug, Clone, Error, Facet)]
#[repr(u8)]
pub enum RecordError {
    #[error("File exists and ovewrite option is set to false on the builder")]
    FileExists,
    #[error("Error spawning GDB process")]
    GdbProcessSpawn,
    #[error("Error opening recording file")]
    FileError,
    #[error("Error deserializing recording")]
    DeserializeError,
}

#[derive(Debug, Clone, Facet)]
pub struct Recording {
    path: String,
    name: String,
    ids: u64,
    gdb_options: gdb::process::Builder,
    gdb_output: Vec<(u64, String, MiRecord)>,
    user_input: Vec<(u64, String)>,
    dibbuk_event: Vec<(u64, DebuggerEvent)>,
    dibbuk_command: Vec<(u64, DebuggerCommand)>,
}

impl Recording {
    pub fn push_gdb_output(&mut self, out: String, mi: MiRecord) -> u64 {
        let id = self.ids;
        self.gdb_output.push((id, out, mi));
        self.ids += 1;
        id
    }

    pub fn push_user_input(&mut self, input: String) -> u64 {
        let id = self.ids;
        self.user_input.push((id, input));
        self.ids += 1;
        id
    }

    pub fn push_dibbuk_event(&mut self, source: u64, evt: DebuggerEvent) {
        // skip Tick events, as it gets very verbose
        if let DebuggerEvent::Tick = evt {
            return;
        }

        self.dibbuk_event.push((source, evt));
    }

    pub fn push_dibbuk_command(&mut self, source: u64, cmd: DebuggerCommand) {
        self.dibbuk_command.push((source, cmd));
    }

    pub fn stop(self) -> Result<(), std::io::Error> {
        let r = self::to_string(&self);

        std::fs::write(self.path, r)
    }

    fn load(input: String) -> Result<Self, RecordError> {
        self::from_str(input.as_str()).map_err(|_| RecordError::DeserializeError)
    }

    pub fn into_mi_test_cases(self) -> Vec<(String, MiRecord)> {
        self.gdb_output
            .into_iter()
            .map(|(_, x, y)| (x, y))
            .collect()
    }

    // pub fn store()
}

pub struct RecordingBuilder {
    path: String,
    name: String,
    overwrite: bool,
}

impl RecordingBuilder {
    pub fn new() -> Self {
        RecordingBuilder {
            path: String::new(),
            name: String::new(),
            overwrite: false,
        }
    }

    pub fn path(mut self, p: impl Into<String>) -> Self {
        self.path = p.into();
        self
    }

    pub fn overwrite(mut self, opt: bool) -> Self {
        self.overwrite = opt;
        self
    }

    fn path_exists(&self) -> bool {
        std::fs::exists(&self.path).unwrap()
    }

    pub fn load(self) -> Result<Recording, RecordError> {
        let str = std::fs::read_to_string(self.path).map_err(|_| RecordError::FileExists)?;
        let rec = Recording::load(str)?;
        Ok(rec)
    }

    pub fn build(
        self,
        gdb_builder: gdb::process::Builder,
    ) -> Result<(GdbHandle, Recording), RecordError> {
        if !self.overwrite && self.path_exists() {
            return Err(RecordError::FileExists);
        }

        let gdb_options = gdb_builder.clone();
        let gdb_handle = gdb_builder
            .spawn()
            .map_err(|_| RecordError::GdbProcessSpawn)?;
        Ok((
            gdb_handle,
            Recording {
                ids: 0,
                path: self.path,
                name: self.name,
                gdb_options,
                gdb_output: Vec::new(),
                user_input: Vec::new(),
                dibbuk_event: Vec::new(),
                dibbuk_command: Vec::new(),
            },
        ))
    }
}

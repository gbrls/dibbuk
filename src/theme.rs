use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;

use crate::tui::Id;

pub fn border_focus(focus: bool) -> Style {
    match focus {
        true => Style::default().white().bold(),
        false => Style::default().dark_gray(),
    }
}

pub fn memory_permissions(r: bool, w: bool, x: bool) -> Style {
    match (r, w, x) {
        (true, true, true) => Style::default().bold().red(),
        (true, true, false) => Style::default().blue(),
        (true, false, true) => Style::default().yellow().bold(),
        (true, false, false) => Style::default().dark_gray(),
        _ => Style::default(),
    }
}

pub struct UILayout {
    pub unused: Vec<Rect>,
    pub sections: HashMap<Id, Rect>,
}

impl UILayout {
    pub fn new(base: Rect) -> Self {
        UILayout {
            unused: vec![base],
            sections: HashMap::new(),
        }
    }

    pub fn base(self) -> Self {
        let mut unused = self.unused;
        let mut sections = self.sections;

        let root = unused.pop().unwrap();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Max(3),         // input
                    Constraint::Percentage(92), // ...
                    Constraint::Max(1),         // statusbar
                ]
                .as_ref(),
            )
            .split(root);

        sections.insert(Id::GDbUserInput, chunks[0]);
        sections.insert(Id::Help, chunks[2]);
        unused.push(chunks[1]);

        UILayout { unused, sections }
    }

    pub fn main(self) -> Self {
        let mut unused = self.unused;
        let mut sections = self.sections;
        let root = unused.pop().unwrap();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(55), //  disasm
                    Constraint::Percentage(45), //  regs / logs
                ]
                .as_ref(),
            )
            .split(root);

        let vchunks_right = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(40), //  regs
                    Constraint::Percentage(40), //  memory
                    Constraint::Percentage(20), //  logs
                ]
                .as_ref(),
            )
            .split(chunks[1]);

        let vchunks_left = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(85), //  disasm
                    Constraint::Percentage(15), //  bt
                ]
                .as_ref(),
            )
            .split(chunks[0]);

        sections.insert(Id::Disassembly, vchunks_left[0]);
        sections.insert(Id::Callstack, vchunks_left[1]);
        sections.insert(Id::Registers, vchunks_right[0]);
        sections.insert(Id::MemoryProbes, vchunks_right[1]);
        sections.insert(Id::Logs, vchunks_right[2]);

        UILayout { unused, sections }
    }

    pub fn fill(self, id: Id) -> Self {
        let mut unused = self.unused;
        let mut sections = self.sections;
        let root = unused.pop().unwrap();

        sections.insert(id, root);

        UILayout { unused, sections }
    }

    pub fn add_blank(self, id: Id) -> Self {
        let mut unused = self.unused;
        let mut sections = self.sections;

        let rect = sections.get(&id).unwrap();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Max(20), Constraint::Percentage(80)].as_ref())
            .split(*rect);

        sections.insert(id, chunks[0]);

        UILayout { unused, sections }
    }
}

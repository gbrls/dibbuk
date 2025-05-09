use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;

use crate::tui::Id;

const ZDEFAULT: usize = 1;
const ZPOPUP: usize = 4;

pub fn border_focus(focus: bool) -> Style {
    match focus {
        true => Style::default().white().bold(),
        false => Style::default().dark_gray(),
    }
}

pub fn memory_permissions(r: bool, w: bool, x: bool) -> Style {
    match (r, w, x) {
        (_, true, true) => Style::default().bold().red(),
        (true, true, false) => Style::default().blue(),
        (true, false, true) => Style::default().yellow().bold(),
        (true, false, false) => Style::default().light_magenta(),
        (false, false, true) => Style::default().italic().red(),
        (false, true, false) => Style::default().italic().red(),
        _ => Style::default(),
    }
}

pub struct UILayout {
    pub unused: Vec<Rect>,
    pub sections: Vec<HashMap<Id, Rect>>,
}

impl UILayout {
    pub fn new(base: Rect) -> Self {
        UILayout {
            unused: vec![base],
            sections: vec![
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            ],
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
                    Constraint::Percentage(92), // ...
                    Constraint::Max(1),         // statusbar
                ]
                .as_ref(),
            )
            .split(root);

        sections[ZPOPUP].insert(Id::HelpPopup, root);
        sections[ZDEFAULT].insert(Id::Statusline, chunks[1]);
        unused.push(chunks[0]);

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
                    Constraint::Percentage(40), //  disasm
                    Constraint::Percentage(60), //  regs / logs
                ]
                .as_ref(),
            )
            .split(root);

        let vchunks_right = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(60), //  regs
                    Constraint::Percentage(20), //  memory
                    Constraint::Percentage(20), //  logs
                    Constraint::Min(3),         // input
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

        sections[ZDEFAULT].insert(Id::Disassembly, vchunks_left[0]);
        sections[ZDEFAULT].insert(Id::Callstack, vchunks_left[1]);
        sections[ZDEFAULT].insert(Id::Registers, vchunks_right[0]);
        sections[ZDEFAULT].insert(Id::MemoryProbes, vchunks_right[1]);
        sections[ZDEFAULT].insert(Id::Logs, vchunks_right[2]);

        sections[ZDEFAULT].insert(Id::GDbUserInput, vchunks_right[3]);
        //sections[ZDEFAULT].insert(Id::GDbUserInput, vchunks_right[2]);

        UILayout { unused, sections }
    }

    pub fn fill(self, id: Id) -> Self {
        let mut unused = self.unused;
        let mut sections = self.sections;
        let root = unused.pop().unwrap();

        sections[4].insert(id, root);

        UILayout { unused, sections }
    }

    pub fn add_blank(self, id: Id) -> Self {
        let mut unused = self.unused;
        let mut sections = self.sections;

        let rect = sections[4].get(&id).unwrap();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Max(20), Constraint::Percentage(80)].as_ref())
            .split(*rect);

        sections[4].insert(id, chunks[0]);

        UILayout { unused, sections }
    }
}

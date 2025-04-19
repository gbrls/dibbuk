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
                    Constraint::Percentage(80), // ...
                    Constraint::Max(3),         // statusbar
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
                    Constraint::Min(10),        // left
                    Constraint::Percentage(35), //  regs
                    Constraint::Percentage(40), //  disasm
                ]
                .as_ref(),
            )
            .split(root);

        sections.insert(Id::Logs, chunks[0]);
        sections.insert(Id::Registers, chunks[1]);
        sections.insert(Id::Disassembly, chunks[2]);

        UILayout { unused, sections }
    }

    pub fn fill(self, id: Id) -> Self {
        let mut unused = self.unused;
        let mut sections = self.sections;
        let root = unused.pop().unwrap();

        sections.insert(id, root);

        UILayout { unused, sections }
    }
}

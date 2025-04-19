use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn border_focus(focus: bool) -> Style {
    match focus {
        true => Style::default().blue(),
        false => Style::default().dark_gray(),
    }
}

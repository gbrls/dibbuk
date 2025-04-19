use crate::theme;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets;
use ratatui::widgets::*;

pub struct Logs {
    history: Vec<String>,
}

impl Logs {
    pub fn new() -> Self {
        Logs {
            history: Vec::new(),
        }
    }
}

impl crate::ntui::Component for Logs {
    fn view(&self, frame: &mut Frame, rect: Rect, focused: bool) {
        frame.render_widget(
            Paragraph::new(format!("{:#?}", self.history))
                .style(Color::Blue)
                .block(
                    Block::bordered()
                        .title("logs")
                        .style(theme::border_focus(focused)),
                ),
            rect,
        );
    }
    fn handle_app_event(&mut self, event: &crate::AppEvent) {
        self.history.push(format!("{:?}", event));
    }
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::AppDataHandle) {}
}

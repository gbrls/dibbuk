use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets;

pub struct Help {
    contents: String,
}

impl Help {
    pub fn new() -> Self {
        Help {
            contents: String::from("hello?"),
        }
    }
}

impl crate::ntui::Component for Help {
    fn view(&self, frame: &mut Frame, rect: Rect, focused: bool) {
        frame.render_widget(widgets::Paragraph::new(self.contents.as_str()), rect);
    }

    fn handle_terminal_event(&mut self, event: &crossterm::event::Event, app_data_handle: &crate::AppDataHandle) {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            }) => {
                self.contents.push('x');
            }
            _ => {}
        }
    }
    fn handle_app_event(&mut self, event: &crate::AppEvent) {}
}

use crate::tui::{InputMode, ViewMode};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets;

pub struct Help {
    contents: String,
    view_mode: ViewMode,
    input_mode: InputMode,
}

impl Help {
    pub fn new() -> Self {
        Help {
            contents: String::from("hello?"),
            view_mode: ViewMode::default(),
            input_mode: InputMode::default(),
        }
    }
}

impl crate::tui::Component for Help {
    fn view(&mut self, frame: &mut Frame, rect: Rect, focused: bool) {
        let input_mode_color = match self.input_mode {
            InputMode::Insert => Color::Red,
            InputMode::Normal => Color::Blue,
            InputMode::Navigation => Color::LightGreen,
        };

        frame.render_widget(
            widgets::Paragraph::new(format!("[{:?}] : {:?}", self.input_mode, self.view_mode))
                .style(Style::default().fg(Color::Black).bold())
                .bg(input_mode_color),
            rect,
        );
    }

    fn handle_terminal_event(
        &mut self,
        event: &crossterm::event::Event,
        app_data_handle: &crate::AppDataHandle,
    ) {
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
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {
        match event {
            crate::tui::UiEvent::ChangeInputMode(input_mode) => {
                self.input_mode = *input_mode;
            }
            crate::tui::UiEvent::ChangeViewMode(view_mode) => {
                self.view_mode = *view_mode;
            }
            _ => {}
        }
    }
}

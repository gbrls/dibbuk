use crate::process_ui::ProcessState;
use crate::tui::{InputMode, ViewMode, ViewOptions};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets;

pub struct Statusline {
    contents: String,
    view_mode: ViewMode,
    input_mode: InputMode,
}

impl Statusline {
    pub fn new() -> Self {
        Statusline {
            contents: String::from("hello?"),
            view_mode: ViewMode::default(),
            input_mode: InputMode::default(),
        }
    }
}

impl crate::tui::Component for Statusline {
    fn view(
        &mut self,
        process: &mut ProcessState,
        view_options: &ViewOptions,
        frame: &mut Frame,
        rect: Rect,
        focused: bool,
    ) {
        let input_mode_color = match self.input_mode {
            InputMode::Insert => Color::Red,
            InputMode::Normal => Color::Blue,
            InputMode::Navigation => Color::Green,
        };

        frame.render_widget(
            Line::from(vec![
                Span::from(format!(" {:?} ", self.input_mode))
                    .style(Style::default().fg(Color::Black).bg(input_mode_color)),
                Span::from(format!(" | {:?}", self.view_mode)),
                Span::from(format!(" | {:?}", process.environment_cwd)),
            ]),
            rect,
        );
    }

    fn handle_terminal_event(
        &mut self,
        event: &crossterm::event::Event,
        app_data_handle: &crate::TxChannels,
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
    fn handle_app_event(&mut self, event: &crate::AppEvent, app_data_handle: &crate::TxChannels) {}
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {
        match event {
            _ => {}
        }
    }
}

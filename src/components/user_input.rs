use crate::process_ui::ProcessState;
use crate::theme;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets;
use ratatui::widgets::*;

pub struct UserInput {
    history: Vec<String>,
}

impl UserInput {
    pub fn new() -> Self {
        UserInput {
            history: vec![String::from("")],
        }
    }
}

impl crate::tui::Component for UserInput {
    fn view(&mut self, process: &ProcessState, frame: &mut Frame, rect: Rect, focused: bool) {
        frame.render_widget(
            Paragraph::new(self.history.last().unwrap().as_str())
                .style(Color::Red)
                .block(
                    Block::bordered()
                        .title("input")
                        .border_style(theme::border_focus(focused)),
                ),
            rect,
        );
    }
    fn handle_app_event(
        &mut self,
        event: &crate::AppEvent,
        app_data_handle: &crate::AppDataHandle,
    ) {
    }
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {}
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::AppDataHandle) {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                //modifiers: KeyModifiers::NONE,
                ..
            }) => {
                if self.history.is_empty() {
                    self.history.push(String::new());
                }

                self.history.last_mut().unwrap().push(*c);
            }

            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            }) => {
                let mut cmd = self.history.last().unwrap().clone();
                if cmd.is_empty() && self.history.len() >= 2 {
                    cmd = self.history[self.history.len() - 2].clone();
                } else {
                    self.history.push(String::new());
                }

                let tx = crate::process::StdinCommand::Input(cmd.clone());
                app_data_handle.channels.gdb_stdin_tx.send(tx).unwrap();
                app_data_handle
                    .channels
                    .event_tx
                    .send(crate::AppEvent::Log(cmd))
                    .unwrap();
            }

            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            }) => {
                if !self.history.is_empty() && self.history.last().unwrap().len() > 0 {
                    self.history.last_mut().unwrap().pop();
                }
            }

            _ => {}
        }
    }
}

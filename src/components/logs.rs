use crate::gdb::parser::MiRecord;
use crate::process_ui::ProcessState;
use crate::theme;
use crate::tui::ViewOptions;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub struct Logs {
    history: Vec<String>,
    view_mode: LogsMode,
    list_state: ListState,
}

#[derive(Debug)]
enum LogsMode {
    JustConsole,
    Verbose,
}

impl Logs {
    pub fn new() -> Self {
        Logs {
            history: Vec::new(),
            //view_mode: LogsMode::JustConsole,
            view_mode: LogsMode::Verbose,
            list_state: ListState::default(),
        }
    }
}

impl crate::tui::Component for Logs {
    fn view(
        &mut self,
        process: &mut ProcessState,
        view_options: &ViewOptions,
        frame: &mut Frame,
        rect: Rect,
        focused: bool,
    ) {
        let lines = self.history.iter().enumerate().map(|(i, l)| {
            let style = if l.starts_with("GdbMi") {
                Style::default().dark_gray()
            } else if l.starts_with("Gdb") {
                Style::default()
            } else if l.starts_with("~>") {
                Style::default().red()
            } else {
                Style::default().blue()
            };

            match self.view_mode {
                LogsMode::Verbose => ListItem::new(format!("[{}] {}", i, l)).style(style),
                LogsMode::JustConsole => ListItem::new(l.as_str()).style(style),
            }
        });

        frame.render_stateful_widget(
            List::new(lines)
                .highlight_style(Style::default().white().bold())
                .block(
                    Block::bordered()
                        .title(format!("{} - {:?}", "logs", self.view_mode))
                        .style(theme::border_focus(focused)),
                ),
            rect,
            &mut self.list_state,
        );
    }
    fn handle_app_event(
        &mut self,
        event: &crate::AppEvent,
        app_data_handle: &crate::AppDataHandle,
    ) {
        match event {
            crate::AppEvent::GdbMi(MiRecord::ConsoleStream(s)) => {
                self.history
                    .push(format!("{}", s.replace("\n", "").replace("\t", "    ")));
                self.list_state.select_last();
            }

            crate::AppEvent::Log(log) => {
                self.history.push(String::new());
                self.history.push(format!("~> {}", log));
                self.list_state.select_last();
            }

            any => match self.view_mode {
                LogsMode::Verbose => {
                    self.history.push(format!("{:?}", any));
                    self.list_state.select_last();
                }
                _ => {}
            },
            _ => {}
        }
        //self.history.push(format!("{:?}", event));
    }
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::AppDataHandle) {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            }) => {
                self.view_mode = match self.view_mode {
                    LogsMode::Verbose => LogsMode::JustConsole,
                    LogsMode::JustConsole => LogsMode::Verbose,
                };
            }
            _ => {}
        }
    }

    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {}
}

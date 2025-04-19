// src/tui.rs

use crate::tui::Msg;
use std::collections::HashMap;

use std::time::{Duration, SystemTime};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::ratatui::{
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Sub, SubClause, SubEventClause, Update,
};

use crate::parser::MiRecord;
use crate::process::StdinCommand;
use crate::tui::keymap;
use crate::AppEvent;
use tuirealm::MockComponent;

use tuirealm::props::{Alignment, TextModifiers};
use tuirealm::{Component, Event, Props, State, StateValue};

#[derive(Debug)]
enum LogsMode {
    JustConsole,
    Verbose,
}

pub struct Logs {
    props: Props,
    logs: Vec<String>,
    view_mode: LogsMode,
    list_state: ListState,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            props: Props::default(),
            logs: Vec::new(),
            view_mode: LogsMode::JustConsole,
            list_state: ListState::default(),
        }
    }
}

impl Logs {
    pub fn text<S>(mut self, s: S) -> Self
    where
        S: AsRef<str>,
    {
        self.attr(Attribute::Text, AttrValue::String(s.as_ref().to_string()));
        self
    }

    pub fn alignment(mut self, a: Alignment) -> Self {
        self.attr(Attribute::TextAlign, AttrValue::Alignment(a));
        self
    }

    pub fn foreground(mut self, c: Color) -> Self {
        self.attr(Attribute::Foreground, AttrValue::Color(c));
        self
    }

    pub fn background(mut self, c: Color) -> Self {
        self.attr(Attribute::Background, AttrValue::Color(c));
        self
    }

    pub fn modifiers(mut self, m: TextModifiers) -> Self {
        self.attr(Attribute::TextProps, AttrValue::TextModifiers(m));
        self
    }
}

impl MockComponent for Logs {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Check if visible
        if self.props.get_or(Attribute::Display, AttrValue::Flag(true)) == AttrValue::Flag(true) {
            // Get properties
            let lines = self.logs.iter().enumerate().map(|(i, l)| {
                let style = if l.starts_with("GdbMi") {
                    Style::default().dark_gray()
                } else if l.starts_with("Gdb") {
                    Style::default()
                } else if l.starts_with("> Input") {
                    Style::default().red()
                } else {
                    Style::default().blue()
                };

                match self.view_mode {
                    LogsMode::Verbose => ListItem::new(format!("[{}] {}", i, l)).style(style),
                    LogsMode::JustConsole => ListItem::new(l.as_str()).style(style),
                }
            });
            let focus = self
                .props
                .get_or(Attribute::Focus, AttrValue::Flag(true))
                .unwrap_flag();
            frame.render_stateful_widget(
                List::new(lines)
                    .highlight_style(Style::default().white().bold())
                    .block(
                        Block::bordered()
                            .title(format!("{} - {:?}", "logs", self.view_mode))
                            .border_style(crate::tui::border_config(focus)),
                    ),
                area,
                &mut self.list_state,
            );
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.props.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.props.set(attr, value);
    }

    fn state(&self) -> State {
        State::Vec(
            self.logs
                .iter()
                .map(|log| StateValue::String(log.into()))
                .collect(),
        )
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Submit => {
                //self.logs.push("(~) submit cmd".into());
                CmdResult::Changed(self.state())
            }
            _ => CmdResult::None,
        }

        //CmdResult::None
    }
}

impl Component<Msg, AppEvent> for Logs {
    fn on(&mut self, e: Event<AppEvent>) -> Option<Msg> {
        let logs_keymap: HashMap<Key, Msg> = [(
            Key::Char('i'),
            Msg::ChangeToMode(crate::tui::InputMode::Insert),
        )]
        .into_iter()
        .collect();
        keymap(&e)
            .or(match e {
                Event::Keyboard(KeyEvent {
                    code: key,
                    modifiers: KeyModifiers::NONE,
                }) => logs_keymap.get(&key).cloned(),
                _ => None,
            })
            .or_else(|| match e {
                Event::Keyboard(KeyEvent {
                    code: Key::Tab,
                    modifiers: KeyModifiers::NONE,
                }) => {
                    self.view_mode = match self.view_mode {
                        LogsMode::Verbose => LogsMode::JustConsole,
                        LogsMode::JustConsole => LogsMode::Verbose,
                    };
                    Some(Msg::Empty)
                }
                Event::Keyboard(KeyEvent {
                    code: Key::Enter,
                    modifiers: KeyModifiers::NONE,
                }) => Some(Msg::GdbInput(StdinCommand::StepInstruction)),

                Event::Keyboard(KeyEvent {
                    code: Key::Char('k'),
                    modifiers: KeyModifiers::NONE,
                }) => {
                    self.list_state.scroll_up_by(1);
                    Some(Msg::Empty)
                }

                Event::Keyboard(KeyEvent {
                    code: Key::Char('u'),
                    modifiers: KeyModifiers::CONTROL,
                }) => {
                    self.list_state.scroll_up_by(16);
                    Some(Msg::Empty)
                }

                Event::Keyboard(KeyEvent {
                    code: Key::Char('d'),
                    modifiers: KeyModifiers::CONTROL,
                }) => {
                    self.list_state.scroll_down_by(16);
                    Some(Msg::Empty)
                }

                Event::Keyboard(KeyEvent {
                    code: Key::Char('j'),
                    modifiers: KeyModifiers::NONE,
                }) => {
                    self.list_state.scroll_down_by(1);
                    Some(Msg::Empty)
                }

                Event::Keyboard(KeyEvent {
                    code: Key::Char('G'),
                    modifiers: KeyModifiers::SHIFT,
                }) => {
                    self.list_state.select_last();
                    Some(Msg::Empty)
                }

                Event::Keyboard(KeyEvent {
                    code: Key::Char('g'),
                    modifiers: KeyModifiers::NONE,
                }) => {
                    self.list_state.select_first();
                    Some(Msg::Empty)
                }

                Event::User(AppEvent::GdbMi(MiRecord::ConsoleStream(s))) => {
                    self.logs
                        .push(format!("{}", s.replace("\n", "").replace("\t", " ")));
                    self.list_state.select_last();
                    Some(Msg::Empty)
                }

                Event::User(AppEvent::Log(s)) => {
                    self.logs
                        .push(format!("{}", s.replace("\n", "").replace("\t", " ")));
                    self.list_state.select_last();
                    Some(Msg::Empty)
                }

                Event::User(app_event) => {
                    match self.view_mode {
                        LogsMode::Verbose => {
                            self.logs.push(format!("{:?}", app_event));
                            self.list_state.select_last();
                        }
                        _ => {}
                    }
                    Some(Msg::Empty)
                }
                _ => None,
            })
    }
}

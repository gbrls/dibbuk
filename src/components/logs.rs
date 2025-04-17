// src/tui.rs

use crate::tui::Msg;

use std::time::{Duration, SystemTime};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::ratatui::{
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};

use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Sub, SubClause, SubEventClause, Update,
};

use crate::AppEvent;
use tuirealm::MockComponent;

use tuirealm::props::{Alignment, TextModifiers};
use tuirealm::{Component, Event, Props, State, StateValue};

pub struct Logs {
    props: Props,
    logs: Vec<String>,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            props: Props::default(),
            logs: Vec::new(),
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
            let lines = self.logs.iter().map(|l| {
                let style = if l.starts_with("GdbMi") {
                    Style::default().dark_gray()
                } else if l.starts_with("Gdb") {
                    Style::default()
                } else {
                    Style::default().blue()
                };
                //let style = Style::default();
                ListItem::new(l.as_str()).style(style)
            });
            let focus = self
                .props
                .get_or(Attribute::Focus, AttrValue::Flag(true))
                .unwrap_flag();
            //println!("{:?}", text);
            let alignment = self
                .props
                .get_or(Attribute::TextAlign, AttrValue::Alignment(Alignment::Left))
                .unwrap_alignment();
            let foreground = self
                .props
                .get_or(Attribute::Foreground, AttrValue::Color(Color::Reset))
                .unwrap_color();
            let background = self
                .props
                .get_or(Attribute::Background, AttrValue::Color(Color::Reset))
                .unwrap_color();
            let modifiers = self
                .props
                .get_or(
                    Attribute::TextProps,
                    AttrValue::TextModifiers(TextModifiers::empty()),
                )
                .unwrap_text_modifiers();
            frame.render_stateful_widget(
                List::new(lines)
                    .highlight_style(Style::default().reversed())
                    .block(Block::bordered().title("logs").border_style(match focus {
                        true => Style::new().blue(),
                        false => Style::new().dark_gray(),
                    })),
                area,
                &mut ratatui::widgets::ListState::default().with_selected(Some(self.logs.len())),
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
        let cmd = match e {
            Event::Keyboard(KeyEvent {
                code: Key::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            }) => return Some(Msg::Quit),

            Event::Keyboard(KeyEvent {
                code: Key::Enter,
                modifiers: KeyModifiers::NONE,
            }) => return Some(Msg::GdbInput(crate::process::StdinCommand::StepInstruction)),

            Event::Keyboard(KeyEvent {
                code: Key::Char('?'),
                modifiers: KeyModifiers::NONE,
            }) => {
                //Cmd::Submit
                return Some(Msg::ShowHelp);
            }
            Event::Keyboard(_) => Cmd::Submit,
            Event::User(AppEvent::GdbMi(crate::parser::MiRecord::ConsoleStream(s))) => {
                self.logs.push(format!("{}", s));
                Cmd::Submit
            }
            Event::User(app_event) => {
                self.logs.push(format!("{:?}", app_event));
                Cmd::Submit
            }

            // default
            _ => Cmd::None,
        };

        match self.perform(cmd) {
            CmdResult::Changed(_) => Some(Msg::Log),
            _ => Some(Msg::Empty),
        }
        // Does nothing
        //None
    }
}

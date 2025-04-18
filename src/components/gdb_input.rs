use crate::tui::Msg;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::ratatui::{prelude::*, widgets::*, Frame, Terminal};

use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Sub, SubClause, SubEventClause, Update,
};

use tuirealm::event;

use tuirealm::terminal::{CrosstermTerminalAdapter, TerminalAdapter, TerminalBridge};

use crate::AppEvent;
use tuirealm::MockComponent;

use tuirealm::props::{Alignment, TextModifiers};
use tuirealm::{Component, Event, Props, State, StateValue};

pub struct GdbInput {
    props: Props,
    messages: Vec<String>,
}

impl Default for GdbInput {
    fn default() -> Self {
        Self {
            props: Props::default(),
            messages: Vec::new(),
        }
    }
}

impl GdbInput {
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

    fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

impl MockComponent for GdbInput {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Check if visible
        if self.props.get_or(Attribute::Display, AttrValue::Flag(true)) == AttrValue::Flag(true) {
            let dstr = String::from("???");
            let text = self.messages.last().unwrap_or(&dstr);
            let text = format!("{}", text);

            // Get properties
            let focus = self
                .props
                .get_or(Attribute::Focus, AttrValue::Flag(true))
                .unwrap_flag();
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
            frame.render_widget(
                Paragraph::new(text.as_str()).style(Color::Yellow).block(
                    Block::bordered()
                        .title("input")
                        .border_style(crate::tui::border_config(focus)),
                ),
                area,
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
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            _ => CmdResult::None,
        }
    }
}

impl Component<Msg, AppEvent> for GdbInput {
    fn on(&mut self, e: Event<AppEvent>) -> Option<Msg> {
        let help_keymap: HashMap<Key, Msg> =
            [(Key::Esc, Msg::ChangeToMode(crate::tui::InputMode::Normal))]
                .into_iter()
                .collect();

        //crate::tui::keymap(&e)
        (match e {
            Event::Keyboard(KeyEvent {
                code: key,
                modifiers: KeyModifiers::NONE,
            }) => help_keymap.get(&key).cloned(),
            _ => None,
        })
        .or_else(|| match e {
            Event::Keyboard(KeyEvent {
                code: event::Key::Enter,
                modifiers: KeyModifiers::NONE,
            }) => {
                let mut cmd = self.messages.last().unwrap().clone();
                if cmd.is_empty() {
                    cmd = self.messages[self.messages.len() - 2].clone();
                } else {
                    self.messages.push(String::new());
                }
                Some(Msg::GdbInput(crate::process::StdinCommand::Input(cmd)))
            }

            Event::Keyboard(KeyEvent {
                code: event::Key::Backspace,
                modifiers: KeyModifiers::NONE,
            }) => {
                if !self.messages.is_empty() && self.messages.last().unwrap().len() > 0 {
                    self.messages.last_mut().unwrap().pop();
                }
                Some(Msg::Empty)
            }

            Event::Keyboard(KeyEvent {
                code: event::Key::Char(c),
                modifiers: KeyModifiers::NONE,
            }) => {
                if self.messages.is_empty() {
                    self.messages.push(String::new());
                }

                self.messages.last_mut().unwrap().push(c);

                //None
                Some(Msg::Empty)
                //Some(Msg::Quit)
            }

            _ => None,
        })
        .or(crate::tui::keymap(&e))
    }
}

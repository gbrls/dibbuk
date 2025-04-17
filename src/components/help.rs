use crate::tui::Msg;
use std::time::{Duration, SystemTime};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::ratatui::{prelude::*, widgets::*, Frame, Terminal};

use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Sub, SubClause, SubEventClause, Update,
};

use tuirealm::terminal::{CrosstermTerminalAdapter, TerminalAdapter, TerminalBridge};

use crate::AppEvent;
use tuirealm::MockComponent;

use tuirealm::props::{Alignment, TextModifiers};
use tuirealm::{Component, Event, Props, State, StateValue};

pub struct Help {
    props: Props,
}

impl Default for Help {
    fn default() -> Self {
        Self {
            props: Props::default(),
        }
    }
}

impl Help {
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

impl MockComponent for Help {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Check if visible
        if self.props.get_or(Attribute::Display, AttrValue::Flag(true)) == AttrValue::Flag(true) {
            // Get properties
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
            frame.render_widget(
                Paragraph::new(
                    "global: ? for help, Esc to exit\nlogs: kj to up and down, / for search",
                )
                .block(
                    Block::bordered()
                        .title("help - press any key to exit")
                        .border_type(BorderType::Rounded)
                        .title_alignment(Alignment::Center)
                        .border_style(match focus {
                            true => Style::new().blue(),
                            false => Style::new().dark_gray(),
                        }), //.bg(Style::default().blue()),
                ),
                Help::centered_rect(frame.area(), 40, 40),
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

impl Component<Msg, AppEvent> for Help {
    fn on(&mut self, e: Event<AppEvent>) -> Option<Msg> {
        match e {
            Event::Keyboard(KeyEvent { .. }) => Some(Msg::HideHelp),
            _ => None,
        }
    }
}

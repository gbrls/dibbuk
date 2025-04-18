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

enum HelpView {
    Default,
    Extra,
}

pub struct Help {
    props: Props,
    state: HelpView,
}

impl Default for Help {
    fn default() -> Self {
        Self {
            props: Props::default(),
            state: HelpView::Default,
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
        if self.props.get_or(Attribute::Display, AttrValue::Flag(true)) == AttrValue::Flag(true) {
            let focus = self
                .props
                .get_or(Attribute::Focus, AttrValue::Flag(true))
                .unwrap_flag();
            frame.render_widget(
                Paragraph::new(match self.state {
                    HelpView::Extra => "extra tips!",
                    HelpView::Default => {
                        "global: ? for help, Esc to exit\nlogs: kj to up and down, / for search"
                    }
                })
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
        use tuirealm::event::Key;
        match e {
            Event::Keyboard(KeyEvent {
                code: Key::Tab,
                modifiers: KeyModifiers::NONE,
            }) => {
                self.state = match self.state {
                    HelpView::Default => HelpView::Extra,
                    HelpView::Extra => HelpView::Default,
                };
                Some(Msg::Empty)
            }
            Event::FocusLost => Some(Msg::HideHelp),
            Event::Keyboard(KeyEvent { .. }) => Some(Msg::HideHelp),
            _ => None,
        }
        .or(crate::tui::keymap(&e))
    }
}

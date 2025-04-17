// src/tui.rs

use futures_util::FutureExt;

use tuirealm::listener::{ListenerResult, Poll};

use tuirealm::application::PollStrategy;

use crate::components::*;
use crate::{mi2command, process};
use color_eyre::Result;
use crossterm;

use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind};
use futures_util::stream::StreamExt; // Required for EventStream::next()
use ratatui::{
    backend::Backend,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::time::{Duration, SystemTime};
use tuirealm::command::{Cmd, CmdResult};

use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Sub, SubClause, SubEventClause, Update,
};

use std::sync::Arc;
use tokio::select; // Use tokio's select macro
use tokio::sync::{broadcast, mpsc};
use tuirealm::terminal::{CrosstermTerminalAdapter, TerminalAdapter, TerminalBridge};

use tuirealm::event::NoUserEvent;
use tuirealm::MockComponent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiState {
    Default,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    Logs,
    Help,
    Registers,
    Disassembly,
    Welcome,
    GDbUserInput,
}

use tuirealm::props::{Alignment, TextModifiers};
use tuirealm::{Component, Event, Props, State};

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Quit,
    Empty,
    Log,
    ShowHelp,
    HideHelp,
    GdbInput(process::StdinCommand),
}

pub struct Model<T>
where
    T: TerminalAdapter,
{
    /// Application
    pub app: Application<Id, Msg, AppEvent>,
    /// Indicates that the application must quit
    pub quit: bool,
    /// Tells whether to redraw interface
    pub redraw: bool,
    /// Used to draw to terminal
    pub terminal: TerminalBridge<T>,
    pub app_data_handle: crate::AppDataHandle,
    pub ui_state: UiState,
}

impl Model<CrosstermTerminalAdapter> {
    fn new(app_data: crate::AppDataHandle) -> Self {
        Self {
            app: Self::init_app(app_data.channels.event_tx.clone()),
            quit: false,
            redraw: true,
            terminal: TerminalBridge::init_crossterm().expect("Cannot initialize terminal"),
            app_data_handle: app_data,
            ui_state: UiState::Default,
        }
    }
}

impl<T> Model<T>
where
    T: TerminalAdapter,
{
    pub fn view(&mut self) {
        use tuirealm::ratatui::layout::{Constraint, Direction, Layout};
        assert!(self
            .terminal
            .draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .margin(1)
                    .constraints(
                        [
                            Constraint::Min(1),  // Clock
                            Constraint::Max(60), // Letter Counter
                            Constraint::Max(80), // Clock
                        ]
                        .as_ref(),
                    )
                    .split(f.area());

                let left_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Constraint::Max(4), // Clock
                            Constraint::Min(1), // Clock
                        ]
                        .as_ref(),
                    )
                    .split(chunks[0]);

                match self.ui_state {
                    UiState::Default => {
                        self.app.view(&Id::Logs, f, left_chunks[1]);
                        self.app.view(&Id::GDbUserInput, f, left_chunks[0]);
                        self.app.view(&Id::Registers, f, chunks[1]);
                        self.app.view(&Id::Disassembly, f, chunks[2]);
                        //self.app.view(&Id::Help, f, chunks[0]);
                    }

                    UiState::Help => {
                        self.app.view(&Id::Logs, f, left_chunks[1]);
                        self.app.view(&Id::GDbUserInput, f, left_chunks[0]);
                        self.app.view(&Id::Registers, f, chunks[1]);
                        self.app.view(&Id::Disassembly, f, chunks[2]);
                        self.app.view(&Id::Help, f, chunks[0]);
                    }
                }
            })
            .is_ok());
    }
    fn init_app(app_events_rx: broadcast::Sender<AppEvent>) -> Application<Id, Msg, AppEvent> {
        let mut app: Application<Id, Msg, AppEvent> = Application::init(
            EventListenerCfg::default()
                .crossterm_input_listener(Duration::from_millis(20), 3)
                .poll_timeout(Duration::from_millis(10))
                .tick_interval(Duration::from_secs(1))
                .add_port(
                    Box::new(BroadcastPoller::new(&app_events_rx)),
                    Duration::from_millis(50),
                    5,
                ),
        );

        assert!(app
            .mount(
                Id::Logs,
                Box::new(
                    Logs::default()
                        .text("Waiting for a Msg...")
                        .alignment(Alignment::Left)
                        .background(Color::Reset)
                        .foreground(Color::LightYellow)
                        .modifiers(TextModifiers::BOLD),
                ),
                Vec::default(),
            )
            .is_ok());

        assert!(app
            .mount(Id::Help, Box::new(Help::default()), Vec::default())
            .is_ok());

        assert!(app
            .mount(
                Id::GDbUserInput,
                Box::new(GdbInput::default()),
                Vec::default()
            )
            .is_ok());

        assert!(app
            .mount(
                Id::Registers,
                Box::new(Registers::default()),
                Vec::default()
            )
            .is_ok());

        assert!(app
            .subscribe(
                &Id::Registers,
                Sub::new(SubEventClause::User(AppEvent::Any), SubClause::Always)
            )
            .is_ok());

        assert!(app
            .mount(
                Id::Disassembly,
                Box::new(Disassembly::default()),
                Vec::default()
            )
            .is_ok());

        assert!(app
            .subscribe(
                &Id::Disassembly,
                Sub::new(SubEventClause::User(AppEvent::Any), SubClause::Always)
            )
            .is_ok());

        assert!(app
            .subscribe(
                &Id::Logs,
                Sub::new(SubEventClause::User(AppEvent::Any), SubClause::Always)
            )
            .is_ok());

        // active!
        //assert!(app.active(&Id::Help).is_ok());
        //assert!(app.active(&Id::Logs).is_ok());
        assert!(app.active(&Id::GDbUserInput).is_ok());
        app
    }
}

impl<T> Update<Msg> for Model<T>
where
    T: TerminalAdapter,
{
    fn update(&mut self, msg: Option<Msg>) -> Option<Msg> {
        if let Some(msg) = msg {
            // Set redraw
            self.redraw = true;
            // Match message
            match msg {
                Msg::Quit => {
                    self.quit = true;
                    None
                }
                Msg::Empty => None,
                Msg::Log => {
                    //println!("log!!!!!!!");
                    None
                }
                Msg::ShowHelp => {
                    assert!(self.app.active(&Id::Help).is_ok());
                    self.ui_state = UiState::Help;
                    None
                }

                Msg::HideHelp => {
                    assert!(self.app.blur().is_ok());
                    self.ui_state = UiState::Default;
                    None
                }

                Msg::GdbInput(cmd) => {
                    println!("input!!!!!!!");
                    tokio::spawn({
                        let gdb_command_tx = self.app_data_handle.channels.gdb_stdin_tx.clone();
                        async move {
                            gdb_command_tx
                                .send(cmd)
                                .unwrap();
                        }
                    });
                    Some(Msg::Empty)
                }
            }
        } else {
            None
        }
    }
}

use crate::AppEvent;

pub struct BroadcastPoller {
    receiver: broadcast::Receiver<AppEvent>,
}

impl BroadcastPoller {
    pub fn new(sender: &broadcast::Sender<AppEvent>) -> Self {
        Self {
            receiver: sender.subscribe(),
        }
    }

    pub fn from_receiver(receiver: broadcast::Receiver<AppEvent>) -> Self {
        Self { receiver }
    }
}

impl Poll<AppEvent> for BroadcastPoller {
    fn poll(&mut self) -> ListenerResult<Option<Event<AppEvent>>> {
        match self.receiver.try_recv() {
            Ok(user_event) => Ok(Some(Event::User(user_event))),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Lagged(count)) => {
                eprintln!("WARN: Broadcast receiver lagged by {} messages.", count);
                Ok(None)
            }
            Err(broadcast::error::TryRecvError::Closed) => Ok(None),
        }
    }
}

fn blocking_start(app_data: crate::AppDataHandle) {
    let mut model = Model::new(app_data);

    let _ = model.terminal.enter_alternate_screen();
    let _ = model.terminal.enable_raw_mode();

    while !model.quit {
        // Tick
        match model.app.tick(PollStrategy::Once) {
            Err(err) => {
                //assert!(model
                //    .app
                //    .attr(
                //        &Id::Logs,
                //        Attribute::Text,
                //        AttrValue::String(format!("Application error: {}", err)),
                //    )
                //    .is_ok());
            }
            Ok(messages) if messages.len() > 0 => {
                model.redraw = true;
                for msg in messages.into_iter() {
                    let mut msg = Some(msg);
                    while msg.is_some() {
                        msg = model.update(msg);
                    }
                }
            }
            _ => {}
        }

        // Redraw
        if model.redraw {
            model.view();
            model.redraw = false;
        }
    }
    // Terminate terminal
    let _ = model.terminal.restore();
}

/// Represents the possible outcomes of handling an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppReturn {
    Continue,
    Exit,
}

pub async fn run(app_data: crate::AppDataHandle) {
    blocking_start(app_data);
}

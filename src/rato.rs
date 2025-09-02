//

use anyhow::{Context, Result};
use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, MouseEvent};
use futures::{FutureExt, StreamExt};
use std::time::Duration;
use steel::SteelVal;
use steel_derive::Steel;
use tokio::sync::mpsc;

use facet::Facet;
use facet_json::{from_str, to_string};

pub const SHIFT: u8 = 0b0000_0001;
pub const CONTROL: u8 = 0b0000_0010;
pub const ALT: u8 = 0b0000_0100;
pub const SUPER: u8 = 0b0000_1000;
pub const HYPER: u8 = 0b0001_0000;
pub const META: u8 = 0b0010_0000;
pub const NONE: u8 = 0b0000_0000;

#[derive(Clone, Debug, Facet, Steel)]
#[repr(u8)]
pub enum ControlKeys {
    Return,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Escape,
    Unknown,
}

impl From<KeyCode> for ControlKeys {
    fn from(value: KeyCode) -> Self {
        use ControlKeys::*;
        match value {
            KeyCode::Backspace => Backspace,
            KeyCode::Enter => Return,
            KeyCode::Left => Left,
            KeyCode::Right => Right,
            KeyCode::Up => Up,
            KeyCode::Down => Down,
            // KeyCode::Home => todo!(),
            // KeyCode::End => todo!(),
            // KeyCode::PageUp => todo!(),
            // KeyCode::PageDown => todo!(),
            KeyCode::Tab => Tab,
            // KeyCode::BackTab => todo!(),
            // KeyCode::Delete => todo!(),
            // KeyCode::Insert => todo!(),
            // KeyCode::F(_) => todo!(),
            // KeyCode::Char(_) => todo!(),
            // KeyCode::Null => todo!(),
            KeyCode::Esc => Escape,
            // KeyCode::CapsLock => todo!(),
            // KeyCode::ScrollLock => todo!(),
            // KeyCode::NumLock => todo!(),
            // KeyCode::PrintScreen => todo!(),
            // KeyCode::Pause => todo!(),
            // KeyCode::Menu => todo!(),
            // KeyCode::KeypadBegin => todo!(),
            // KeyCode::Media(media_key_code) => todo!(),
            // KeyCode::Modifier(modifier_key_code) => todo!(),
            _ => Unknown,
        }
    }
}

#[derive(Clone, Debug, Facet, Steel)]
#[repr(u8)]
pub enum Update {
    Tick,
    Resize,
}

#[derive(Clone, Debug, Facet, Steel)]
#[repr(u8)]
pub enum TermEvent {
    TerminalUpdate(Update),
    Key(char, u8),
    ControlKey(ControlKeys, u8),
    Unknown,
}

impl TermEvent {
    pub fn is_tick(e: &TermEvent) -> bool {
        matches!(e, TermEvent::TerminalUpdate(Update::Tick))
    }
}

impl From<CrosstermEvent> for TermEvent {
    fn from(value: CrosstermEvent) -> Self {
        match value {
            CrosstermEvent::FocusGained => TermEvent::Unknown,
            CrosstermEvent::FocusLost => TermEvent::Unknown,
            CrosstermEvent::Key(KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            }) => TermEvent::Key(c, modifiers.bits()),

            CrosstermEvent::Key(KeyEvent {
                code, modifiers, ..
            }) => TermEvent::ControlKey(code.into(), modifiers.bits()),
            CrosstermEvent::Mouse(mouse_event) => TermEvent::Unknown,
            CrosstermEvent::Paste(_) => TermEvent::Unknown,
            CrosstermEvent::Resize(_, _) => TermEvent::Unknown,
            CrosstermEvent::Key(_) => TermEvent::Unknown,
        }
    }
}

impl Into<SteelVal> for TermEvent {
    fn into(self) -> SteelVal {
        SteelVal::StringV(self::to_string(&self).into())
    }
}

/// Terminal event handler.
#[allow(dead_code)]
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<TermEvent>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<TermEvent>,
    /// Event handler thread.
    handler: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    pub fn new(tick_rate: u64) -> Self {
        let tick_rate = Duration::from_millis(tick_rate);
        let (sender, receiver) = mpsc::unbounded_channel();
        let _sender = sender.clone();
        let handler = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick = tokio::time::interval(tick_rate);
            loop {
                let tick_delay = tick.tick();
                let crossterm_event = reader.next().fuse();
                tokio::select! {
                  _ = _sender.closed() => {
                    break;
                  }
                  _ = tick_delay => {
                    _sender.send(TermEvent::TerminalUpdate(Update::Tick)).unwrap();
                  }
                  Some(Ok(evt)) = crossterm_event => {
                    _sender.send(evt.into()).unwrap();
                  }
                };
            }
        });
        Self {
            sender,
            receiver,
            handler,
        }
    }

    /// Receive the next event from the handler thread.
    ///
    /// This function will always block the current thread if
    /// there is no data available and it's possible for more data to be sent.
    pub async fn next(&mut self) -> Result<TermEvent> {
        self.receiver.recv().await.context("Event handler errror")
    }
}

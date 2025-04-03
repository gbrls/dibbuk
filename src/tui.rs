// src/tui.rs

use futures_util::FutureExt;

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
use std::sync::Arc;
use tokio::select; // Use tokio's select macro
use tokio::sync::{broadcast, mpsc};

/// Represents the possible outcomes of handling an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppReturn {
    Continue,
    Exit,
}

pub async fn run(app_data: crate::AppDataHandle) {
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut app = App::new(app_data);

    crossterm::terminal::enable_raw_mode().unwrap();
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        //crossterm::cursor::Hide
        crossterm::event::EnableMouseCapture
    )
    .unwrap();

    app.run(&mut terminal).await;
}

/// Application state
pub struct App {
    /// Input history
    inputs: Vec<String>,
    /// Current value of the input box
    input: String,
    /// Position of cursor in the input box.
    character_index: usize,
    /// Current input mode
    input_mode: InputMode,
    /// History of commands sent and GDB output received
    messages: Vec<String>, // Combine GDB output and commands here for simplicity
    //
    app_data: crate::AppDataHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,  // Not editing input
    Editing, // Editing input
}

impl App {
    pub fn new(app_data: crate::AppDataHandle) -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::Normal,
            messages: Vec::new(),
            inputs: Vec::new(),
            character_index: 0,
            app_data,
        }
    }

    /// Runs the main event loop.
    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let tick_rate = std::time::Duration::from_millis(10);
        let mut crossterm_event_stream = event::EventStream::new();
        let mut reader = crossterm::event::EventStream::new();
        let mut interval = tokio::time::interval(tick_rate);

        loop {
            let tick = interval.tick();
            let crossterm_event = reader.next().fuse();
            select! {
                maybe_event = crossterm_event => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            if self.handle_crossterm_event(event)? == AppReturn::Exit {
                                break; // Exit the loop
                            }
                        }
                        Some(Err(e)) => {
                            println!("error reading event::{:?}\r", e);
                             break;
                        }
                        None => break,
                    }
                },
              _ = tick => {
                    let r = self.app_data.state.read().await;
                    terminal.draw(|frame| self.draw(frame, r))?;
              },

                event = self.app_data.channels.event_rx.recv().fuse() => {
                self.messages.push(format!("[evt] {:?}", event));
            }

                // Handle GDB output events
                result = self.app_data.channels.gdb_mi_rx.recv().fuse() => {
                    match result {
                        Ok(gdb_event) => {
                            // Handle the GDB output event
                            self.handle_gdb_event(gdb_event);
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // Handle lagged receiver - GDB produced messages faster than we consumed
                            // You might want to log this, clear some state, or ignore it.
                            self.messages.push(format!("[Error: UI lagged by {} GDB messages]", n));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // GDB output channel was closed. The GDB task likely exited.
                            self.messages.push("[Info: GDB output channel closed]".to_string());
                            // Decide if the app should exit here or just stop listening.
                            // break; // Example: Exit if GDB closes
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Handles events received from Crossterm (user input, resizes).
    /// Returns `Ok(AppReturn::Exit)` if the application should quit.
    fn handle_crossterm_event(&mut self, event: CrosstermEvent) -> Result<AppReturn> {
        if let CrosstermEvent::Key(key) = event {
            // Handle key events based on input mode
            match self.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('e') | KeyCode::Char('i') => {
                        self.input_mode = InputMode::Editing;
                    }
                    KeyCode::Char('q') => {
                        return Ok(AppReturn::Exit); // Signal to exit
                    }
                    // Add other Normal mode keybindings here (e.g., scrolling, focus change)
                    _ => {}
                },

                InputMode::Editing if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Enter => self.submit_input(),
                    KeyCode::Char(to_insert) => self.enter_char(to_insert),
                    KeyCode::Backspace => self.delete_char(),
                    KeyCode::Left => self.move_cursor_left(),
                    KeyCode::Right => self.move_cursor_right(),
                    KeyCode::Up => {
                        if self.inputs.len() > 0 {
                            self.input = self.inputs.last().unwrap().clone();
                        }
                    }
                    KeyCode::Esc => {
                        self.input_mode = InputMode::Normal;
                    }
                    // Add other Editing mode keybindings (Home, End, Delete, etc.)
                    _ => {}
                },
                _ => {} // Ignore other key events like releases or repeats if not needed
            }
        } else if let CrosstermEvent::Resize(_, _) = event {
            // Handle resize events if necessary (Ratatui handles basic redrawing)
        }
        // Other CrosstermEvents (Mouse, Focus, Paste) can be handled here

        Ok(AppReturn::Continue) // Continue running
    }

    /// Handles events received from the GDB process.
    fn handle_gdb_event(&mut self, event: process::MiOutput) {
        let message = match event {
            process::MiOutput {
                mi: Some(crate::parser::MiRecord::ConsoleStream(s)),
                ..
            } => s,

            process::MiOutput {
                mi: Some(crate::parser::MiRecord::Unknown(s)),
                ..
            } => format!("{}", s),
            process::MiOutput { mi: Some(mi), .. } => format!("[mi] {:?}", mi),
            process::MiOutput { mi: None, string } => format!("[raw] {:?}", string),
        };
        self.messages.push(message);
        // Optionally, update other state based on the GDB event (e.g., status bar)
    }

    // --- Input Handling Methods (mostly unchanged) ---

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        if self.character_index > 0 {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    /// Sends the current input as a command to GDB.
    fn submit_input(&mut self) {
        let command_text = self.input.trim(); // Trim whitespace
        if !command_text.is_empty() {
            // Add the submitted command to the message history for feedback
            self.messages.push(format!("> {}", command_text));

            // Send the command to the GDB handler task
            let cmd = process::StdinCommand::Input(command_text.to_string());
            if let Err(e) = self.app_data.channels.gdb_stdin_tx.send(cmd) {
                // Handle error sending command (e.g., GDB task died)
                self.messages.push(format!("[Error sending to GDB: {}]", e));
            }

            self.inputs.push(command_text.to_string().clone());
        } else if self.inputs.len() > 0 {
            let command_text = self.inputs.last().unwrap();
            self.messages.push(format!("> {}", command_text));

            // Send the command to the GDB handler task
            let cmd = process::StdinCommand::Input(command_text.to_string());
            if let Err(e) = self.app_data.channels.gdb_stdin_tx.send(cmd) {
                // Handle error sending command (e.g., GDB task died)
                self.messages.push(format!("[Error sending to GDB: {}]", e));
            }

            self.inputs.push(command_text.clone());
        }
        // Clear the input field and reset cursor regardless of whether send was successful
        self.input.clear();
        self.reset_cursor();
    }

    // --- Rendering Logic ---

    fn draw(&self, frame: &mut Frame, app_state: tokio::sync::RwLockReadGuard<crate::AppState>) {
        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ]);
        let [help_area, input_area, messages_area] = vertical.areas(frame.area());

        let [messages_area, state_area] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Min(1)])
                .areas(messages_area);

        let (msg, style) = match self.input_mode {
            InputMode::Normal => (
                vec![
                    format!("{:?} - ", app_state.gdb_ctx.state).into(),
                    "Press ".into(),
                    "q".bold(),
                    " to exit, ".into(),
                    "e".bold(),
                    " to start editing.".bold(),
                ],
                Style::default().add_modifier(Modifier::RAPID_BLINK),
            ),
            InputMode::Editing => (
                vec![
                    format!("{:?} - ", app_state.gdb_ctx.state).into(),
                    "Press ".into(),
                    "Esc".bold(),
                    " to stop editing, ".into(),
                    "Enter".bold(),
                    " to record the message".into(),
                ],
                Style::default(),
            ),
        };
        let text = Text::from(Line::from(msg)).patch_style(style);
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, help_area);

        let state_message = Paragraph::new(format!("{:#?}", app_state)).scroll((525, 0));
        frame.render_widget(state_message, state_area);

        let input = Paragraph::new(self.input.as_str())
            .style(match self.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Editing => Style::default().fg(Color::Yellow),
            })
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);
        match self.input_mode {
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            InputMode::Normal => {}

            // Make the cursor visible and ask ratatui to put it at the specified coordinates after
            // rendering
            #[allow(clippy::cast_possible_truncation)]
            InputMode::Editing => frame.set_cursor_position(Position::new(
                // Draw the cursor at the current position in the input field.
                // This position is can be controlled via the left and right arrow key
                input_area.x + self.character_index as u16 + 1,
                // Move one line down, from the border to the input line
                input_area.y + 1,
            )),
        }

        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .map(|m| {
                let content = Line::from(Span::raw(format!("{m}")));
                ListItem::new(content).style(Style::default().fg(match m {
                    m if m.starts_with("[mi]") => Color::Blue,
                    m if m.starts_with("[evt]") => Color::Yellow,
                    _ => Color::White,
                }))
            })
            .collect();
        let messages = List::new(messages).block(Block::bordered().title("Messages"));
        frame.render_widget(messages, messages_area);
    }
    /// Renders the UI frame.
    fn render(&self, frame: &mut Frame) {
        let main_layout = Layout::vertical([
            Constraint::Length(1), // Help text
            Constraint::Length(3), // Input box
            Constraint::Min(1),    // Messages/Output
        ]);
        let [help_area, input_area, messages_area] = main_layout.areas(frame.size());

        self.render_help_message(frame, help_area);
        self.render_input_box(frame, input_area);
        self.render_messages(frame, messages_area);
    }

    fn render_help_message(&self, frame: &mut Frame, area: Rect) {
        let (msg, style) = match self.input_mode {
            InputMode::Normal => (
                vec![
                    "Press ".into(),
                    "q".bold(),
                    " to exit, ".into(),
                    "e".bold(),
                    " or ".into(),
                    "i".bold(),
                    " to edit input.".bold(),
                ],
                Style::default(), //.add_modifier(Modifier::RAPID_BLINK), // Blinking can be annoying
            ),
            InputMode::Editing => (
                vec![
                    "Press ".into(),
                    "Esc".bold(),
                    " to stop editing, ".into(),
                    "Enter".bold(),
                    " to send command.".into(),
                ],
                Style::default(),
            ),
        };
        let text = Text::from(Line::from(msg)).patch_style(style);
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, area);
    }

    fn render_input_box(&self, frame: &mut Frame, area: Rect) {
        let input = Paragraph::new(self.input.as_str())
            .style(match self.input_mode {
                InputMode::Normal => Style::default().fg(Color::DarkGray), // Dim when not editing
                InputMode::Editing => Style::default().fg(Color::Yellow),
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("GDB Command Input"),
            );
        frame.render_widget(input, area);

        // Set cursor position only when editing
        if self.input_mode == InputMode::Editing {
            // Calculate cursor position carefully based on UTF-8 characters
            let cursor_x = self
                .input
                .chars()
                .take(self.character_index)
                .map(|c| 8 as u16) // TODO: this is ?????
                .sum::<u16>();

            #[allow(clippy::cast_possible_truncation)]
            // Usize -> u16, safe for typical terminal widths
            frame.set_cursor_position(Position::new(
                area.x + 1 + cursor_x, // +1 for border
                area.y + 1,            // +1 for border
            ));
        }
    }

    fn render_messages(&self, frame: &mut Frame, area: Rect) {
        // Create ListItems from the messages buffer
        let message_items: Vec<ListItem> = self
            .messages
            .iter()
            .map(|m| {
                // Simple styling based on prefix (could be more sophisticated)
                let style = if m.starts_with('>') {
                    Style::default().fg(Color::Cyan) // User command
                } else if m.starts_with("<GDB") {
                    Style::default().fg(Color::Green) // GDB output
                } else if m.starts_with("[Error") {
                    Style::default().fg(Color::Red) // Internal errors
                } else {
                    Style::default() // Default
                };
                ListItem::new(Line::from(Span::styled(m, style)))
            })
            .collect();

        // Create the List widget
        let messages_list = List::new(message_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("GDB Log / Output"),
            )
            .direction(ratatui::widgets::ListDirection::BottomToTop); // Show newest at bottom

        frame.render_widget(messages_list, area);

        // Optional: Scroll handling could be added here using ListState
    }
}

// Helper for character width (basic version, assumes simple characters) - Consider unicode-width crate for accuracy
//trait CharWidth {
//    fn width(&self) -> usize;
//}
//impl CharWidth for char {
//    fn width(&self) -> usize {
//        // Very basic width calculation, might not be accurate for all unicode
//        if unicode_width::UnicodeWidthChar::width(*self).is_some() {
//            unicode_width::UnicodeWidthChar::width(*self).unwrap_or(1)
//        } else {
//            1 // Default to 1 if width cannot be determined
//        }
//    }
//}
//


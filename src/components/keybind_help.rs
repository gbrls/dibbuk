use crate::process_ui::ProcessState;
use crate::tui::{InputMode, ViewMode, ViewOptions};
use color_eyre::owo_colors::OwoColorize;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{self, Block, Clear, List, ListItem, Paragraph};

pub struct HelpPopup {
    contents: String,
    view_mode: ViewMode,
    input_mode: InputMode,
}

impl HelpPopup {
    pub fn new() -> Self {
        HelpPopup {
            contents: String::from("hello?"),
            view_mode: ViewMode::default(),
            input_mode: InputMode::default(),
        }
    }
}

impl crate::tui::Component for HelpPopup {
    fn view(
        &mut self,
        process: &mut ProcessState,
        view_options: &ViewOptions,
        frame: &mut Frame,
        rect: Rect,
        focused: bool,
    ) {
        if !focused {
            return;
        }

        let rect = rect.inner(Margin::new(30, 20));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(3), Constraint::Percentage(80)].as_ref())
            .split(rect);

        Clear.render(rect, frame.buffer_mut());
        frame.render_widget(
            Paragraph::new("welcome to dibbuk!")
                .centered()
                .bold()
                .white()
                .block(Block::bordered().border_style(Style::new().blue())),
            //Span::from("Welcome to dibbuk!"),
            chunks[0],
        );

        frame.render_widget(
            List::new(vec![
                ListItem::new(""),
                ListItem::new("  > Dibbuk have multiple input modes. It's shown in the statusbar on the bottom of the screen.\n"),
                ListItem::new("When in input mode, dibbuk will send the keyboard input directly to gdb, the input bar at the top will be highlighted."),
                ListItem::new("To exit input mode, press Escape."),
            ]),
            chunks[1].inner(Margin::new(4, 1)),
        );

        frame.render_widget(Block::bordered().border_style(Style::new().blue()), rect)
    }

    fn handle_terminal_event(
        &mut self,
        event: &crossterm::event::Event,
        app_data_handle: &crate::AppDataHandle,
    ) {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            }) => {
                self.contents.push('x');
            }
            _ => {}
        }
    }
    fn handle_app_event(
        &mut self,
        event: &crate::AppEvent,
        app_data_handle: &crate::AppDataHandle,
    ) {
    }
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {
        match event {
            _ => {}
        }
    }
}

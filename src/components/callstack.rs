use crate::mi2command::StackFrame;
use crate::tui::{InputMode, ViewMode};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub struct CallStack {
    stack_frames: Vec<StackFrame>,
}

impl CallStack {
    pub fn new() -> Self {
        CallStack {
            stack_frames: Vec::new(),
        }
    }
}

impl crate::tui::Component for CallStack {
    fn view(&mut self, frame: &mut Frame, rect: Rect, focused: bool) {
        let frames = self.stack_frames.iter().map(|f| {
            Line::from(vec![
                //Span::from(format!("{} ", f.depth)),
                Span::from(format!("{:#018x} ", f.addr)),
                Span::from(format!("{}", f.function.clone().unwrap_or("??".to_owned())))
                    .style(Style::default().dark_gray()),
            ])
        });

        frame.render_stateful_widget(
            List::new(frames)
                .block(
                    Block::bordered()
                        .title("call stack")
                        .border_style(crate::theme::border_focus(focused)),
                )
                .highlight_style(Style::default().bold())
                .highlight_symbol("> "),
            rect,
            &mut ListState::default().with_selected(Some(0)),
        );
    }

    fn handle_terminal_event(
        &mut self,
        event: &crossterm::event::Event,
        app_data_handle: &crate::AppDataHandle,
    ) {
    }
    fn handle_app_event(&mut self, event: &crate::AppEvent) {
        match event {
            crate::AppEvent::Gdb(crate::mi2command::GdbMessage::StackFrames(frames)) => {
                self.stack_frames = frames.clone();
                self.stack_frames.sort_by_key(|f| f.depth);
            }
            _ => {}
        }
    }
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {}
}

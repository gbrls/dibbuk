use crate::components::display_u64;
use crate::process_ui::ProcessState;
use crate::tui::{InputMode, ViewMode, ViewOptions};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub struct CallStack {}

impl CallStack {
    pub fn new() -> Self {
        CallStack {}
    }
}

impl crate::tui::Component for CallStack {
    fn view(
        &mut self,
        process: &mut ProcessState,
        view_options: &ViewOptions,
        frame: &mut Frame,
        rect: Rect,
        focused: bool,
    ) {
        if process.frames.is_none() {
            return;
        }

        let frames = process.frames.as_ref().unwrap().iter().map(|f| {
            Line::from(vec![
                display_u64(f.addr, process, view_options),
                Span::from(format!(
                    " {}",
                    f.function.clone().unwrap_or("??".to_owned())
                ))
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
    fn handle_app_event(
        &mut self,
        event: &crate::AppEvent,
        app_data_handle: &crate::AppDataHandle,
    ) {
    }
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {}
}

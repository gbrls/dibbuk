use crate::mi2command::StackFrame;
use crate::process_ui::ProcessState;
use crate::tui::{InputMode, ViewMode};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::fmt::Display;

pub(crate) struct Joiner<'a, 'b, T: Display> {
    sep: &'a str,
    list: &'b [T],
}

impl<'a, 'b, T: Display> Joiner<'a, 'b, T> {
    pub(crate) fn new(sep: &'a str, list: &'b [T]) -> Joiner<'a, 'b, T> {
        Joiner { sep, list }
    }
}

impl<'a, 'b, T: Display> Display for Joiner<'a, 'b, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut iter = self.list.iter();
        if let Some(first) = iter.next() {
            write!(f, "{}", first)?;
            for item in iter {
                write!(f, "{}{}", self.sep, item)?;
            }
        }
        Ok(())
    }
}

pub struct MemoryProbes {}

impl MemoryProbes {
    pub fn new() -> Self {
        MemoryProbes {}
    }
}

fn bytes_u8(mem: &[u8]) -> String {
    let items: Vec<_> = mem
        .iter()
        .map(|b| {
            let mut s = format!("{b:#2x}");
            s.drain(..2);
            s
        })
        .collect();
    format!("{}", Joiner::new(" ", &items))
}

impl crate::tui::Component for MemoryProbes {
    fn view(&mut self, process: &mut ProcessState, frame: &mut Frame, rect: Rect, focused: bool) {
        let items = process
            .memory_probes
            .iter()
            .flat_map(|(name, (addr, mem))| {
                let v = Vec::new();
                let (r, w, x) = process.addr_memory_perm(*addr).unwrap();
                let tele = process.telescope(*addr, v);
                if tele.is_none() {
                    vec![]
                } else {
                    let tele = tele.unwrap();
                    let tele_len = tele.len();

                    let tele: Vec<_> = tele
                        .into_iter()
                        .enumerate()
                        .flat_map(|(i, maybe_addr)| {
                            let style = match process.addr_memory_perm(maybe_addr) {
                                Some((r, w, x)) => crate::theme::memory_permissions(r, w, x),
                                None => Style::default(),
                            };

                            if i == (tele_len - 1) {
                                vec![Span::from(format!("{maybe_addr:#02x}")).style(style)]
                            } else if i == 0 {
                                vec![
                                    Span::from(format!("{name}: ")).style(Style::default()),
                                    Span::from(format!("{maybe_addr:#018x}")).style(style),
                                    Span::from(" > ").style(Style::default()),
                                ]
                            } else {
                                vec![
                                    Span::from(format!("{maybe_addr:#018x}")).style(style),
                                    Span::from(" > ").style(Style::default()),
                                ]
                            }
                        })
                        .collect();

                    vec![ListItem::from(Line::from(tele))]
                }
            });
        frame.render_widget(
            List::new(items).block(
                Block::bordered()
                    .title("memory view")
                    .border_style(crate::theme::border_focus(focused)),
            ),
            rect,
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

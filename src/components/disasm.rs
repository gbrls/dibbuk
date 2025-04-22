use crate::process_ui::ProcessState;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::time::{Duration, SystemTime};

pub struct Disasm {}

impl Disasm {
    pub fn new() -> Self {
        Disasm {}
    }
}

impl crate::tui::Component for Disasm {
    fn view(&mut self, process: &ProcessState, frame: &mut Frame, rect: Rect, focused: bool) {
        let instruction_pointer = process.registers.get("rip").cloned();

        let header_cells = ["Address", "Asm"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().bold()));
        let header = Row::new(header_cells)
            .style(Style::default().blue())
            .height(1)
            .bottom_margin(1);

        let mut addrs: Vec<_> = process
            .disassembly
            .iter()
            .filter(|(addr, _)| addr.abs_diff(instruction_pointer.unwrap_or(0)) < 64)
            .collect();
        addrs.sort_by(|(addr0, asm0), (addr1, asm1)| addr0.cmp(addr1));

        let selected = {
            let mut ret = None;
            for (i, (addr, _)) in addrs.iter().enumerate() {
                if instruction_pointer.unwrap_or(0) == **addr {
                    ret = Some(i);
                }
            }
            ret
        };

        let rows = addrs.iter().map(|(addr, disasm)| {
            let formatted_value = format!("{}", disasm.str);
            let fmt_addr = format!("{:#018x} {}+{:#05x}", addr, disasm.func, disasm.offset);

            let style = match instruction_pointer {
                Some(rip) if rip > **addr => Style::default().dark_gray(),
                Some(rip) if rip == **addr => Style::default().yellow(),
                Some(_) => Style::default().white(),
                None => Style::default(),
            };

            let cells = vec![Cell::from(fmt_addr), Cell::from(formatted_value)];
            Row::new(cells).height(1).style(style)
        });

        let widths = [Constraint::Min(8), Constraint::Min(20)];

        let title = format!(
            "disassembly {:#018x}",
            instruction_pointer.unwrap_or(0)
        );

        let register_table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title.as_str())
                    .border_style(crate::theme::border_focus(focused)),
            )
            .column_spacing(2)
            .row_highlight_style(Style::default().bold());

        let mut table_state = ratatui::widgets::TableState::default().with_selected(selected);
        frame.render_stateful_widget(register_table, rect, &mut table_state)
    }
    fn handle_app_event(
        &mut self,
        event: &crate::AppEvent,
        app_data_handle: &crate::AppDataHandle,
    ) {
    }
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::AppDataHandle) {}
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {}
}

use crate::components::display_u64;
use crate::components::telescope;
use crate::process_ui::ProcessState;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::time::{Duration, SystemTime};

pub struct NRegisters {}
impl NRegisters {
    pub fn new() -> Self {
        NRegisters {}
    }
}

impl crate::tui::Component for NRegisters {
    fn view(&mut self, process: &mut ProcessState, frame: &mut Frame, rect: Rect, focused: bool) {
        let register_names = process.registers.keys();
        let state_message = Paragraph::new(format!(
            "rip -> {:#x}",
            process.registers.get("rip").unwrap_or(&0)
        ));

        let common_x64_registers = [
            "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15", "rip", "eflags",
        ];

        use ratatui::widgets::{Cell, Row, Table};
        let header_cells = ["Register", "Value?", "Mem"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().bold()));
        let header = Row::new(header_cells)
            .style(Style::default().blue())
            .height(1)
            .bottom_margin(1);

        let rows = common_x64_registers.iter().flat_map(|&reg_name| {
            let value_or_addr = process.registers.get(reg_name).copied().unwrap_or(0);

            let maybe_range = process.addr_memory_map(value_or_addr);

            let mem = if maybe_range.is_some()
                && maybe_range.as_ref().unwrap().map_range.filename().is_some()
            {
                format!(
                    "{} {} +{:#04x}",
                    maybe_range.as_ref().unwrap().map_range.flags,
                    maybe_range
                        .as_ref()
                        .unwrap()
                        .map_range
                        .filename()
                        .unwrap()
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    (value_or_addr as usize) - maybe_range.as_ref().unwrap().map_range.start()
                )
            } else {
                String::from("")
            };

            let cells = vec![
                Cell::from(reg_name).style(Style::default().white().bold()),
                Cell::from(display_u64(value_or_addr, process)),
                Cell::from(mem).style(Style::default().dark_gray()),
            ];

            let tele = process.telescope(value_or_addr, vec![]);
            if tele.is_some() {
                let tele = tele.unwrap();
                let tele = telescope(tele, &process, false, "  └ ".into());
                vec![
                    Row::new(cells).height(1),
                    Row::new(vec![Cell::from(String::new()), Cell::from(tele).dim()]),
                ]
            } else {
                vec![Row::new(cells).height(1)]
            }
        });

        let widths = [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(32),
        ];

        let register_table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Registers (x64)")
                    .border_style(crate::theme::border_focus(focused)),
            )
            .column_spacing(2);

        frame.render_widget(register_table, rect)
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

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::time::{Duration, SystemTime};

pub struct NRegisters {
    value: std::collections::HashMap<String, u64>,
    memory_maps: Vec<crate::mi2command::MemMap>,
}
impl NRegisters {
    pub fn new() -> Self {
        NRegisters {
            value: std::collections::HashMap::new(),
            memory_maps: Vec::new(),
        }
    }
}

impl crate::tui::Component for NRegisters {
    fn view(&mut self, frame: &mut Frame, rect: Rect, focused: bool) {
        let register_names = self.value.keys();
        let state_message =
            Paragraph::new(format!("rip -> {:#x}", self.value.get("rip").unwrap_or(&0)));

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

        let rows = common_x64_registers.iter().map(|&reg_name| {
            let value = self.value.get(reg_name).copied().unwrap_or(0);

            let maybe_range = self.memory_maps.iter().find(|map| {
                let value = value as usize;
                value >= map.map_range.start()
                    && (value < map.map_range.start() + map.map_range.size())
            });

            let style = if maybe_range.is_some() {
                let r = maybe_range.unwrap().map_range.is_read();
                let w = maybe_range.unwrap().map_range.is_write();
                let x = maybe_range.unwrap().map_range.is_exec();
                match (r, w, x) {
                    (true, true, true) => Style::default().bold().red(),
                    (true, true, false) => Style::default().blue(),
                    (true, false, true) => Style::default().yellow().bold(),
                    (true, false, false) => Style::default().dark_gray(),
                    _ => Style::default(),
                }
            } else {
                Style::default()
            };

            let formatted_value = format!("{:#018x}", value);
            let mem =
                if maybe_range.is_some() && maybe_range.unwrap().map_range.filename().is_some() {
                    format!(
                        "{} {} +{:#04x}",
                        maybe_range.unwrap().map_range.flags,
                        maybe_range
                            .unwrap()
                            .map_range
                            .filename()
                            .unwrap()
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap(),
                        (value as usize) - maybe_range.unwrap().map_range.start()
                    )
                } else {
                    String::from("")
                };

            let cells = vec![
                Cell::from(reg_name),
                Cell::from(formatted_value).style(style),
                Cell::from(mem).style(Style::default().dark_gray()),
            ];
            Row::new(cells).height(1)
        });

        let widths = [
            Constraint::Length(8),
            Constraint::Length(20),
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
    fn handle_app_event(&mut self, event: &crate::AppEvent) {
        match event {
            crate::AppEvent::Gdb(crate::mi2command::GdbMessage::RegisterValue(regs)) => {
                for (k, v) in regs.iter() {
                    self.value.insert(k.clone(), *v);
                }
            }

            crate::AppEvent::Gdb(crate::mi2command::GdbMessage::Maps(maps)) => {
                self.memory_maps = maps.clone();
            }

            _ => {}
        }
    }

    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {}
}

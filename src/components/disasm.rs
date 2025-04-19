use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::time::{Duration, SystemTime};

pub struct Disasm {
    value: std::collections::HashMap<usize, crate::mi2command::Disassembly>,
    instruction_pointer: Option<u64>,
}

impl Disasm {
    pub fn new() -> Self {
        Disasm {
            value: std::collections::HashMap::new(),
            instruction_pointer: None,
        }
    }
}

impl crate::tui::Component for Disasm {
    fn view(&mut self, frame: &mut Frame, rect: Rect, focused: bool) {
        let register_names = self.value.keys();

        let header_cells = ["Address", "Asm"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().bold()));
        let header = Row::new(header_cells)
            .style(Style::default().blue())
            .height(1)
            .bottom_margin(1);

        let mut rows: Vec<_> = self.value.iter().collect();
        rows.sort_by(|(addr0, asm0), (addr1, asm1)| addr0.cmp(addr1));

        let selected = {
            let mut ret = None;
            for (i, (addr, _)) in rows.iter().enumerate() {
                if (self.instruction_pointer.unwrap_or(0) as usize) == **addr {
                    ret = Some(i);
                }
            }
            ret
        };

        let rows = rows.iter().map(|(addr, disasm)| {
            let formatted_value = format!("{}", disasm.str);
            let fmt_addr = format!("{:#018x} {}+{:#05x}", addr, disasm.func, disasm.offset);

            let cells = vec![Cell::from(fmt_addr), Cell::from(formatted_value)];
            Row::new(cells).height(1)
        });

        let widths = [Constraint::Min(8), Constraint::Min(20)];

        let title = format!(
            "disassembly {:#018x}",
            self.instruction_pointer.unwrap_or(0)
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
            .row_highlight_style(Style::default().reversed());

        let mut table_state = ratatui::widgets::TableState::default().with_selected(selected);
        frame.render_stateful_widget(register_table, rect, &mut table_state)
    }
    fn handle_app_event(&mut self, event: &crate::AppEvent) {
        match event {
            crate::AppEvent::Gdb(crate::mi2command::GdbMessage::DisassemblyNative(asm_lines)) => {
                for asm in asm_lines.iter() {
                    self.value.insert(asm.addr, asm.clone());
                }
            }

            crate::AppEvent::Gdb(crate::mi2command::GdbMessage::RegisterValue(regs)) => {
                for (k, v) in regs.iter() {
                    if k == "rip" {
                        self.instruction_pointer = Some(*v);
                    }
                }
            }

            _ => {}
        }
    }
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::AppDataHandle) {}
}

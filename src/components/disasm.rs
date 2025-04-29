use crate::process_ui::ProcessState;
use crate::Disassembly;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;
use std::fmt::format;
use std::time::{Duration, SystemTime};

pub struct Disasm {}

impl Disasm {
    pub fn new() -> Self {
        Disasm {}
    }
}

fn instructions_view_window(
    disassembly: &HashMap<u64, Disassembly>,
    instruction_pointer: u64,
) -> Vec<(u64, Disassembly)> {
    let mut addrs: Vec<_> = disassembly
        .iter()
        .filter(|(addr, _)| addr.abs_diff(instruction_pointer) < 256)
        .map(|(addr, asm)| (*addr, asm.clone()))
        .collect();

    addrs.sort_by(|(addr0, asm0), (addr1, asm1)| addr0.cmp(addr1));

    let rip_idx = addrs
        .iter()
        .enumerate()
        .find_map(|(i, (addr, _))| {
            if *addr == instruction_pointer {
                Some(i)
            } else {
                None
            }
        })
        .unwrap_or(0);
    let before_rip_view = 5;
    let to_skip = if rip_idx >= before_rip_view {
        rip_idx - before_rip_view
    } else {
        0
    };

    addrs.into_iter().skip(to_skip).collect()
}

impl crate::tui::Component for Disasm {
    fn view(&mut self, process: &mut ProcessState, frame: &mut Frame, rect: Rect, focused: bool) {
        let instruction_pointer = process.registers.get("rip").cloned();

        let (addrs, cs_addrs) = if instruction_pointer.is_some() {
            (
                instructions_view_window(&process.disassembly, instruction_pointer.unwrap()),
                instructions_view_window(&process.cs_disassembly, instruction_pointer.unwrap()),
            )
        } else {
            (vec![], vec![])
        };

        let selected = {
            let mut ret = None;
            for (i, (addr, _)) in cs_addrs.iter().enumerate() {
                if instruction_pointer.unwrap_or(0) == *addr {
                    ret = Some(i);
                }
            }
            ret
        };

        let meta = format!("{}/{}", addrs.len(), cs_addrs.len());
        let header_cells = ["Address".into(), meta, "operand".into()]
            .into_iter()
            .map(|h| Cell::from(h).style(Style::default().bold()));

        let header = Row::new(header_cells)
            .style(Style::default().blue())
            .height(1)
            .bottom_margin(1);

        //let rows = addrs.iter().zip(cs_addrs.iter()).enumerate().map(
        //    |(i, ((gdb_addr, gdb_disasm), (addr, disasm)))| {
        let rows = cs_addrs.iter().enumerate().map(|(i, (addr, disasm))| {
            let mnemonic = Line::from(vec![Span::from(format!(
                "{} ",
                disasm.mnemonic.as_ref().unwrap_or(&String::new())
            ))
            .style(Style::default().fg(Color::Green))]);

            let operand = Line::from(vec![Span::from(format!(
                "{}",
                disasm.operand.as_ref().unwrap_or(&String::new())
            ))
            .style(Style::default().fg(Color::White))]);

            let fmt_addr = Line::from(vec![Span::from(format!("{:#018x} ", disasm.offset))]);
            let style = match instruction_pointer {
                Some(rip) if rip > *addr => Style::default().dark_gray(),
                Some(rip) if rip == *addr => Style::default().yellow(),
                Some(_) => Style::default().white(),
                None => Style::default(),
            };

            let cells = vec![
                Cell::from(fmt_addr),
                Cell::from(mnemonic),
                Cell::from(operand),
            ];
            Row::new(cells).height(1).style(style)
        });

        let widths = [Constraint::Max(20), Constraint::Max(8), Constraint::Min(10)];

        let tmp = vec![];
        let top = process
            .frames
            .as_ref()
            .unwrap_or(&tmp)
            .iter()
            .take(1)
            .next();
        let title = if let Some(frame) = top {
            format!(
                "disassembly {}",
                frame.function.as_ref().unwrap_or(&String::from("???"))
            )
        } else {
            format!("disassembly {:#018x}", instruction_pointer.unwrap_or(0))
        };
        //let title = format!("disassembly {:#018x}", instruction_pointer.unwrap_or(0));

        let register_table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title.as_str())
                    .border_style(crate::theme::border_focus(focused)),
            )
            .column_spacing(2)
            .row_highlight_style(Style::default().bold())
            .highlight_symbol("> ");

        let mut table_state = ratatui::widgets::TableState::default().with_selected(selected);
        frame.render_stateful_widget(register_table, rect, &mut table_state);
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

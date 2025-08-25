use crate::components::display_u64;
use crate::il::Disassembly;
use crate::process_ui::ProcessState;
use crate::tui::ViewOptions;
use ratatui::crossterm::event::Event;
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;

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

fn display_operand(s: &str, process: &ProcessState, view_options: &ViewOptions) -> Span<'static> {
    let maybe_base16 = u64::from_str_radix(s.strip_prefix("0x").unwrap_or(""), 16);
    let has_commas = s.contains(",");

    match (has_commas, maybe_base16) {
        (_, Ok(val)) => Span::from(display_u64(val, process, view_options)),
        _ => Span::from(s.to_string()).style(Style::default().fg(Color::White)),
    }
}

impl crate::tui::Component for Disasm {
    fn view(
        &mut self,
        process: &mut ProcessState,
        view_options: &ViewOptions,
        frame: &mut Frame,
        rect: Rect,
        focused: bool,
    ) {
        let instruction_pointer = process.registers.get("rip").cloned();

        let (_, cs_addrs) = if instruction_pointer.is_some() {
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

        let meta = format!("{}", cs_addrs.len());
        let header_cells = ["Address".into(), meta, "operand".into()]
            .into_iter()
            .map(|h| Cell::from(h).style(Style::default().bold()));

        let header = Row::new(header_cells)
            .style(Style::default().blue())
            .height(1)
            .bottom_margin(1);

        let rows = cs_addrs.iter().enumerate().map(|(i, (row_addr, disasm))| {
            let mnemonic = Line::from(vec![
                Span::from(format!(
                    "{} ",
                    disasm.mnemonic.as_ref().unwrap_or(&String::new())
                ))
                .style(Style::default().fg(Color::Green)),
            ]);

            let operand = display_operand(
                disasm.operand.as_ref().unwrap_or(&String::new()).as_str(),
                process,
                view_options,
            );

            let style = match instruction_pointer {
                Some(rip) if rip > *row_addr => Style::default().dim(),
                Some(rip) if rip == *row_addr => Style::default().bold(),
                Some(_) => Style::default(),
                None => Style::default(),
            };

            let meta = match (
                &disasm.mnemonic,
                disasm.operand.as_ref().and_then(|op| {
                    u64::from_str_radix(op.strip_prefix("0x").unwrap_or(""), 16).ok()
                }),
            ) {
                // FIXME: this code is bad
                (Some(s), Some(op_addr)) if (s.as_str() == "call") => {
                    //let symbol = process
                    //    .elfs
                    //    .iter()
                    //    .find_map(|(_, elf)| elf.symbols.get(&addr));

                    let symbol = process
                        .elfs
                        .iter()
                        .flat_map(|(_, elf)| elf.symbols.iter())
                        .fold(None, |acc: Option<(u64, &str)>, (addr, sym)| {
                            if *addr >= op_addr && addr.abs_diff(op_addr) <= 8 {
                                let diff = addr - op_addr;
                                if acc.is_none() || (acc.is_some() && acc.unwrap().0 < *addr) {
                                    Some((diff, sym.as_str()))
                                } else {
                                    acc
                                }
                            } else {
                                acc
                            }
                        });
                    if symbol.is_some() {
                        symbol.unwrap().1.to_string()
                    } else {
                        format!("")
                        //format!("not found")
                        //format!(
                        //    "not found {:?}",
                        //    process
                        //        .elfs
                        //        .iter()
                        //        .map(|(_, elf)| &elf.symbols)
                        //        .collect::<Vec<_>>()
                        //)
                    }
                }
                _ => String::new(),
            };

            let collumns = vec![
                Cell::from(
                    display_u64(disasm.offset as u64, process, view_options)
                        .style(Style::default()),
                ),
                Cell::from(mnemonic),
                Cell::from(operand),
                Cell::from(meta).style(Style::default().blue().bold()),
            ];
            Row::new(collumns).height(1).style(style)
        });

        let widths = [
            Constraint::Max(20),
            Constraint::Max(8),
            Constraint::Min(10),
            Constraint::Min(10),
        ];

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

        let register_table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title.as_str())
                    .border_style(crate::theme::border_focus(focused)),
            )
            .column_spacing(2)
            .highlight_symbol("> ".yellow());

        let mut table_state = ratatui::widgets::TableState::default().with_selected(selected);
        frame.render_stateful_widget(register_table, rect, &mut table_state);
    }
    fn handle_app_event(&mut self, event: &crate::AppEvent, app_data_handle: &crate::TxChannels) {}
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::TxChannels) {}
    fn handle_ui_event(&mut self, event: &crate::tui::UiEvent) {}
}

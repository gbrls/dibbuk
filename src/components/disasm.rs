use crate::tui::Msg;
use crate::AppEvent;
use std::collections::HashMap;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::ratatui::prelude::*;
use tuirealm::ratatui::widgets::*;
use tuirealm::MockComponent;
use tuirealm::{Component, Event, Props, State, StateValue};

use crate::mi2command;
use crate::mi2command::GdbMessage::DisassemblyNative;

use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Sub, SubClause, SubEventClause, Update,
};

pub struct Disassembly {
    props: Props,
    value: HashMap<usize, mi2command::Disassembly>,
    instruction_pointer: Option<u64>,
}

impl Default for Disassembly {
    fn default() -> Self {
        Self {
            props: Props::default(),
            value: HashMap::new(),
            instruction_pointer: None,
        }
    }
}

impl MockComponent for Disassembly {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let register_names = self.value.keys();

        use ratatui::widgets::{Cell, Row, Table};
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
            .block(Block::default().borders(Borders::ALL).title(title.as_str()))
            .column_spacing(2)
            .row_highlight_style(Style::default().reversed());

        let mut table_state = ratatui::widgets::TableState::default().with_selected(selected);
        frame.render_stateful_widget(register_table, area, &mut table_state)
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.props.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.props.set(attr, value);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

impl Component<Msg, AppEvent> for Disassembly {
    fn on(&mut self, e: Event<AppEvent>) -> Option<Msg> {
        match e {
            Event::User(AppEvent::Gdb(DisassemblyNative(asm_lines))) => {
                for asm in asm_lines.iter() {
                    self.value.insert(asm.addr, asm.clone());
                }
                return Some(Msg::Empty);
            }

            Event::User(AppEvent::Gdb(crate::mi2command::GdbMessage::RegisterValue(regs))) => {
                for (k, v) in regs.iter() {
                    if k == "rip" {
                        self.instruction_pointer = Some(*v);
                    }
                }
                return Some(Msg::Empty);
            }

            _ => {}
        }
        None
    }
}

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
}

impl Default for Disassembly {
    fn default() -> Self {
        Self {
            props: Props::default(),
            value: HashMap::new(),
        }
    }
}

impl MockComponent for Disassembly {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let register_names = self.value.keys();

        use ratatui::widgets::{Cell, Row, Table};
        // 2. Create header row
        let header_cells = ["Address", "Asm"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().bold())); // Style header
        let header = Row::new(header_cells)
            .style(Style::default().blue()) // Style header row background/foreground
            .height(1) // Explicit height
            .bottom_margin(1); // Margin below header

        // 3. Create data rows by iterating through the desired registers
        let rows = self.value.iter().map(|(addr, disasm)| {
            // Get the value from your context, default to 0 if not found
            let formatted_value = format!("{}", disasm.str);
            let fmt_addr = format!("{:#018x} {}+{:#05x}", addr, disasm.func, disasm.offset);

            // Create cells for the row
            let cells = vec![Cell::from(fmt_addr), Cell::from(formatted_value)];
            Row::new(cells).height(1) // Each row takes 1 line
        });

        // 4. Define column constraints (widths)
        // Adjust lengths as needed for your layout and register name lengths
        let widths = [
            Constraint::Min(8), // Width for register names (e.g., "rflags")
            Constraint::Min(20),   // Width for "0x" + 16 hex digits + padding
        ];

        // 5. Create the table widget
        let register_table = Table::new(rows, widths) // Pass rows and widths
            .header(header) // Set the header row
            .block(Block::default().borders(Borders::ALL).title("disassembly")) // Add a block with title and borders
            .column_spacing(2); // Add spacing between columns

        frame.render_widget(register_table, area)
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

            _ => {}
        }
        None
    }
}

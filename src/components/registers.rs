use crate::tui::Msg;
use crate::AppEvent;
use std::collections::HashMap;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::ratatui::prelude::*;
use tuirealm::ratatui::widgets::*;
use tuirealm::MockComponent;
use tuirealm::{Component, Event, Props, State, StateValue};

use tuirealm::{
    Application, AttrValue, Attribute, EventListenerCfg, Sub, SubClause, SubEventClause, Update,
};

pub struct Registers {
    props: Props,
    value: HashMap<String, u64>,
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            props: Props::default(),
            value: HashMap::new(),
        }
    }
}

impl MockComponent for Registers {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let register_names = self.value.keys();
        let state_message =
            Paragraph::new(format!("rip -> {:#x}", self.value.get("rip").unwrap_or(&0)));

        let common_x64_registers = [
            // General Purpose Registers
            "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12",
            "r13", "r14", "r15",    // Instruction Pointer
            "rip",    // Flags Register
            "rflags", // or "eflags" if that's what gdb provides
        ];

        use ratatui::widgets::{Cell, Row, Table};
        // 2. Create header row
        let header_cells = ["Register", "Value"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().bold())); // Style header
        let header = Row::new(header_cells)
            .style(Style::default().blue()) // Style header row background/foreground
            .height(1) // Explicit height
            .bottom_margin(1); // Margin below header

        // 3. Create data rows by iterating through the desired registers
        let rows = common_x64_registers.iter().map(|&reg_name| {
            // Get the value from your context, default to 0 if not found
            let value = self.value.get(reg_name).copied().unwrap_or(0);

            // Format the value as padded hex (0x prefix, 16 hex digits for 64-bit)
            // Adjust padding (e.g., {:#x}) if you prefer variable width
            let formatted_value = format!("{:#018x}", value);

            // Create cells for the row
            let cells = vec![Cell::from(reg_name), Cell::from(formatted_value)];
            Row::new(cells).height(1) // Each row takes 1 line
        });

        // 4. Define column constraints (widths)
        // Adjust lengths as needed for your layout and register name lengths
        let widths = [
            Constraint::Length(8),  // Width for register names (e.g., "rflags")
            Constraint::Length(20), // Width for "0x" + 16 hex digits + padding
        ];

        // 5. Create the table widget
        let register_table = Table::new(rows, widths) // Pass rows and widths
            .header(header) // Set the header row
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Registers (x64)"),
            ) // Add a block with title and borders
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

impl Component<Msg, AppEvent> for Registers {
    fn on(&mut self, e: Event<AppEvent>) -> Option<Msg> {
        match e {
            Event::User(AppEvent::Gdb(crate::mi2command::GdbMessage::RegisterValue(regs))) => {
                for (k, v) in regs.iter() {
                    self.value.insert(k.clone(), *v);
                }
                return Some(Msg::Empty)
            }

            _ => {}
        }
        None
    }
}

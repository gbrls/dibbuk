use crate::theme::UILayout;
use futures_util::{FutureExt, StreamExt};
use ratatui::crossterm;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::{select, time};

#[derive(Debug, Copy, Clone, Hash)]
pub enum UiEvent {
    Quit,
    ChangeInputMode(InputMode),
    ChangeViewMode(ViewMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputMode {
    Insert,
    Normal,
    Navigation,
}

impl Default for InputMode {
    fn default() -> Self {
        InputMode::Insert
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewMode {
    Default,
    DebugLogs,
}

impl Default for ViewMode {
    fn default() -> Self {
        ViewMode::Default
    }
}

pub trait Component {
    fn view(&mut self, frame: &mut Frame, rect: Rect, focused: bool);
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::AppDataHandle);
    fn handle_app_event(&mut self, event: &crate::AppEvent);
    fn handle_ui_event(&mut self, event: &UiEvent);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    Logs,
    Help,
    Registers,
    Disassembly,
    Welcome,
    GDbUserInput,
}

struct Model {
    pub app_data: crate::AppDataHandle,
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub view_mode: ViewMode,
    pub components: HashMap<Id, Arc<dyn Component>>,
    pub focus_stack: Vec<Id>,
}
unsafe impl Send for Model {}
unsafe impl Sync for Model {}

impl Model {
    fn new(app_data: crate::AppDataHandle) -> Self {
        use crate::components;
        let components = [
            (
                Id::Help,
                Arc::new(components::help::Help::new()) as Arc<dyn Component>,
            ),
            (
                Id::GDbUserInput,
                Arc::new(components::user_input::UserInput::new()) as Arc<dyn Component>,
            ),
            (
                Id::Logs,
                Arc::new(components::logs::Logs::new()) as Arc<dyn Component>,
            ),
            (
                Id::Registers,
                Arc::new(components::NRegisters::new()) as Arc<dyn Component>,
            ),
            (
                Id::Disassembly,
                Arc::new(components::disasm::Disasm::new()) as Arc<dyn Component>,
            ),
        ]
        .into_iter()
        .collect();

        Model {
            app_data,
            should_quit: false,
            input_mode: InputMode::default(),
            view_mode: ViewMode::default(),
            components,
            focus_stack: Vec::new(),
        }
    }

    /// Only updates the focused element with input from terminal
    fn handle_terminal_event(&mut self, event: Event) -> Option<UiEvent> {
        let focused = self.focus_stack.last();
        match event {
            Event::Key(KeyEvent {
                code, modifiers, ..
            }) => self.keybind(code, modifiers).or_else(|| {
                for (id, mut component) in self.components.iter_mut() {
                    if focused.is_some() && focused.unwrap() == id {
                        Arc::get_mut(&mut component)
                            .unwrap()
                            .handle_terminal_event(&event, &self.app_data);
                    }
                }
                None
            }),
            _ => None,
        }
    }

    /// Updates all elements
    fn handle_app_event(&mut self, event: crate::AppEvent) -> Option<UiEvent> {
        for (id, mut component) in self.components.iter_mut() {
            Arc::get_mut(&mut component)
                .unwrap()
                .handle_app_event(&event);
        }
        None
    }

    fn keybind(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<UiEvent> {
        use InputMode::*;
        use UiEvent::*;
        match (self.input_mode, code, modifiers) {
            (Normal, KeyCode::Char('q'), _) => Some(Quit),
            (Normal, KeyCode::Char('i'), _) => Some(ChangeInputMode(Insert)),
            (Normal, KeyCode::Char('v'), _) => Some(ChangeInputMode(Navigation)),

            (Normal, KeyCode::Char('0'), _) => Some(ChangeViewMode(ViewMode::DebugLogs)),
            (Normal, KeyCode::Char('1'), _) => Some(ChangeViewMode(ViewMode::Default)),

            (Insert, KeyCode::Esc, _) => Some(ChangeInputMode(Normal)),
            (Navigation, KeyCode::Esc, _) => Some(ChangeInputMode(Normal)),

            (Navigation, KeyCode::Char('j'), _) => {
                self.app_data
                    .channels
                    .gdb_stdin_tx
                    .send(crate::process::StdinCommand::NextInstruction)
                    .unwrap();

                None
            }
            (Navigation, KeyCode::Char('l'), _) => {
                self.app_data
                    .channels
                    .gdb_stdin_tx
                    .send(crate::process::StdinCommand::StepInstruction)
                    .unwrap();

                None
            }
            (Navigation, KeyCode::Char('h'), _) => {
                self.app_data
                    .channels
                    .gdb_stdin_tx
                    .send(crate::process::StdinCommand::Finish)
                    .unwrap();

                None
            }
            (Navigation, KeyCode::Enter, _) => {
                self.app_data
                    .channels
                    .gdb_stdin_tx
                    .send(crate::process::StdinCommand::Continue)
                    .unwrap();

                None
            }

            _ => None,
        }
    }

    fn update(&mut self, action: UiEvent) {
        match action {
            UiEvent::Quit => {
                self.should_quit = true;
            }
            UiEvent::ChangeInputMode(InputMode::Insert) => {
                self.input_mode = InputMode::Insert;
                self.unfocus();
                self.focus(&Id::GDbUserInput);
            }

            UiEvent::ChangeInputMode(InputMode::Navigation) => {
                self.input_mode = InputMode::Navigation;
                self.unfocus();
                self.focus(&Id::Disassembly);
            }

            UiEvent::ChangeInputMode(InputMode::Normal) => {
                self.input_mode = InputMode::Normal;
                self.unfocus();
                self.focus(&Id::Logs);
            }

            UiEvent::ChangeViewMode(mode) => {
                self.view_mode = mode;
            }
        }

        for (id, mut component) in self.components.iter_mut() {
            Arc::get_mut(&mut component)
                .unwrap()
                .handle_ui_event(&action);
        }
    }

    fn view_layout(&mut self, frame: &mut Frame, layout: &UILayout) {
        let focused = self.focus_stack.last();
        for (id, rect) in layout.sections.iter() {
            Arc::get_mut(&mut self.components.get_mut(&id).unwrap())
                .unwrap()
                .view(frame, *rect, focused.is_some() && focused.unwrap() == id);
        }
    }

    fn view(&mut self, frame: &mut Frame, rect: Rect) {
        let focused = self.focus_stack.last();

        let ui_layout = UILayout::new(frame.area()).base();
        let ui_layout = match self.view_mode {
            ViewMode::Default => ui_layout.main(),
            ViewMode::DebugLogs => ui_layout.fill(Id::Logs),
        };

        self.view_layout(frame, &ui_layout);
    }

    fn focus(&mut self, id: &Id) {
        if !self.focus_stack.is_empty() && self.focus_stack.last().unwrap() == id {
        } else {
            self.focus_stack.push(*id);
        }
    }

    fn unfocus(&mut self) -> Option<Id> {
        self.focus_stack.pop()
    }
}

pub async fn run(app_data_handle: crate::AppDataHandle) {
    use std::io::stdout;

    // initialization
    let backend = ratatui::backend::CrosstermBackend::new(stdout());
    let mut terminal = ratatui::Terminal::new(backend).unwrap();

    stdout().execute(EnterAlternateScreen).unwrap();
    crossterm::terminal::enable_raw_mode().unwrap();

    main_loop(app_data_handle, terminal).await;

    // cleanup
    crossterm::terminal::disable_raw_mode().unwrap();
    stdout().execute(LeaveAlternateScreen).unwrap();
}

async fn main_loop<B: Backend>(app_data_handle: crate::AppDataHandle, mut term: Terminal<B>) {
    let mut ticker = time::interval(Duration::from_millis(10));
    let mut model = Model::new(app_data_handle);
    let mut term_event_reader = crossterm::event::EventStream::new();

    let mut app_event_rx = model.app_data.channels.event_tx.subscribe();

    model.focus(&Id::GDbUserInput);

    while !model.should_quit {
        let tick = ticker.tick();
        let term_event = term_event_reader.next().fuse();
        select! {
            _ = tick => {
                term.draw(|f| {
                    model.view(f, f.area());
                });
            }
            Some(Ok(term_event)) = term_event => {
                if let Some(action) = model.handle_terminal_event(term_event) {
                    model.update(action);
                }
            }
            Ok(event) = app_event_rx.recv().fuse() => {
                if let Some(action) = model.handle_app_event(event) {
                    model.update(action);
                }
            }
        }
    }
}

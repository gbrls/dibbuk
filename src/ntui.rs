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
pub enum ModelAction {
    Quit,
    ChangeInputMode(InputMode),
    ChangeViewMode(ViewMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputMode {
    Insert,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewMode {
    Default,
    DebugLogs,
}

pub trait Component {
    fn view(&self, frame: &mut Frame, rect: Rect, focused: bool);
    fn handle_terminal_event(&mut self, event: &Event, app_data_handle: &crate::AppDataHandle);
    fn handle_app_event(&mut self, event: &crate::AppEvent);
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
        Model {
            app_data,
            should_quit: false,
            input_mode: InputMode::Insert,
            view_mode: ViewMode::Default,
            components: HashMap::new(),
            focus_stack: Vec::new(),
        }
    }

    /// Only updates the focused element with input from terminal
    fn handle_terminal_event(&mut self, event: Event) -> Option<ModelAction> {
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
    fn handle_app_event(&mut self, event: crate::AppEvent) -> Option<ModelAction> {
        for (id, mut component) in self.components.iter_mut() {
            Arc::get_mut(&mut component)
                .unwrap()
                .handle_app_event(&event);
        }
        None
    }

    fn keybind(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<ModelAction> {
        use InputMode::*;
        use ModelAction::*;
        match (self.input_mode, code, modifiers) {
            (Normal, KeyCode::Char('q'), _) => Some(Quit),
            (Normal, KeyCode::Char('i'), _) => Some(ChangeInputMode(Insert)),

            (Normal, KeyCode::Char('0'), _) => Some(ChangeViewMode(ViewMode::DebugLogs)),
            (Normal, KeyCode::Char('1'), _) => Some(ChangeViewMode(ViewMode::Default)),

            (Insert, KeyCode::Esc, _) => Some(ChangeInputMode(Normal)),
            _ => None,
        }
    }

    fn update(&mut self, action: ModelAction) {
        match action {
            ModelAction::Quit => {
                self.should_quit = true;
            }
            ModelAction::ChangeInputMode(InputMode::Insert) => {
                self.input_mode = InputMode::Insert;
                self.unfocus();
                self.focus(&Id::GDbUserInput);
            }

            ModelAction::ChangeInputMode(InputMode::Normal) => {
                self.input_mode = InputMode::Normal;
                self.unfocus();
                self.focus(&Id::Logs);
            }

            ModelAction::ChangeViewMode(mode) => {
                self.view_mode = mode;
            }
        }
    }

    fn view(&self, frame: &mut Frame, rect: Rect) {
        let focused = self.focus_stack.last();
        match self.view_mode {
            ViewMode::Default => {
                let horizontal_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Constraint::Max(4),         //  input
                            Constraint::Percentage(60), // logs
                            Constraint::Max(4),         //  help
                        ]
                        .as_ref(),
                    )
                    .split(frame.area());

                self.components.get(&Id::GDbUserInput).unwrap().view(
                    frame,
                    horizontal_chunks[0],
                    focused.is_some() && focused.unwrap() == &Id::GDbUserInput,
                );

                self.components.get(&Id::Logs).unwrap().view(
                    frame,
                    horizontal_chunks[1],
                    focused.is_some() && focused.unwrap() == &Id::Logs,
                );
                self.components.get(&Id::Help).unwrap().view(
                    frame,
                    horizontal_chunks[2],
                    focused.is_some() && focused.unwrap() == &Id::Help,
                );
            }
            _ => {}
        }
        //frame.render_widget(
        //    widgets::Paragraph::new(format!("{:?} {:?}", self.view_mode, self.input_mode)),
        //    rect,
        //);
    }

    fn focus(&mut self, id: &Id) {
        if !self.focus_stack.is_empty() && self.focus_stack.last().unwrap() == id {
        } else {
            self.focus_stack.push(*id);
        }
    }

    fn unfocus(&mut self) -> Option<Id> {
        if !self.focus_stack.is_empty() {
            self.focus_stack.pop()
        } else {
            None
        }
    }
}

pub async fn run(app_data_handle: crate::AppDataHandle) {
    use std::io::stdout;

    // initialization
    let backend = ratatui::backend::CrosstermBackend::new(stdout());
    let mut terminal = ratatui::Terminal::new(backend).unwrap();

    stdout().execute(EnterAlternateScreen).unwrap();
    crossterm::terminal::enable_raw_mode().unwrap();

    //time::sleep(std::time::Duration::from_millis(1000)).await;
    main_loop(app_data_handle, terminal).await;

    // cleanup
    crossterm::terminal::disable_raw_mode().unwrap();
    stdout().execute(LeaveAlternateScreen).unwrap();
}

async fn main_loop<B: Backend>(app_data_handle: crate::AppDataHandle, mut term: Terminal<B>) {
    let mut ticker = time::interval(Duration::from_millis(100));
    let mut model = Model::new(app_data_handle);
    let mut term_event_reader = crossterm::event::EventStream::new();

    let mut app_event_rx = model.app_data.channels.event_tx.subscribe();

    model
        .components
        .insert(Id::Help, Arc::new(crate::components::nhelp::Help::new()));

    model.components.insert(
        Id::GDbUserInput,
        Arc::new(crate::components::user_input::UserInput::new()),
    );

    model
        .components
        .insert(Id::Logs, Arc::new(crate::components::nlogs::Logs::new()));

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

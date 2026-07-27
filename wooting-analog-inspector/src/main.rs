use std::{
    collections::{HashMap, VecDeque},
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyCode as TerminalKeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use wooting_analog_inspector::{
    ActionConfirmation, ActiveKey, ConfirmationResult, InspectorAction, IntentEvent, IntentTracker,
    collect_active_keys,
};
use wooting_analog_sdk::{AnalogSdk, AnalogValue, Initialised, KeyCode};

const POLL_RATE_HZ: u64 = 60;
const FRAME_TIME: Duration = Duration::from_micros(1_000_000 / POLL_RATE_HZ);
const MAX_EVENTS: usize = 16;
const CLEAR_LOG_KEYCODE: u16 = 0x1a;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = AnalogSdk::new().initialise()?;
    let mut app = InspectorApp::new(sdk);
    let terminal = setup_terminal()?;
    let result = app.run(terminal);

    restore_terminal()?;
    result
}

struct InspectorApp {
    sdk: AnalogSdk<Initialised>,
    tracker: IntentTracker,
    active_keys: Vec<ActiveKey>,
    events: VecDeque<IntentEvent>,
    action_confirmation: ActionConfirmation,
    suppress_until_released: Option<u16>,
    last_error: Option<String>,
}

impl InspectorApp {
    fn new(sdk: AnalogSdk<Initialised>) -> Self {
        Self {
            sdk,
            tracker: IntentTracker::new(),
            active_keys: Vec::new(),
            events: VecDeque::with_capacity(MAX_EVENTS),
            action_confirmation: ActionConfirmation::default(),
            suppress_until_released: None,
            last_error: None,
        }
    }

    fn run(
        &mut self,
        mut terminal: Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let tick_started = Instant::now();
            self.poll();
            terminal.draw(|frame| self.draw(frame))?;

            let elapsed = tick_started.elapsed();
            let timeout = FRAME_TIME.saturating_sub(elapsed);

            if event::poll(timeout)? && self.handle_terminal_event(event::read()?) {
                break;
            }
        }

        Ok(())
    }

    fn handle_terminal_event(&mut self, event: Event) -> bool {
        let Event::Key(key) = event else {
            return false;
        };

        if key.kind != KeyEventKind::Press {
            return false;
        }

        match key.code {
            TerminalKeyCode::Char('w') | TerminalKeyCode::Char('W') => {
                if self.register_action(InspectorAction::ClearLog) {
                    self.clear_log();
                }

                false
            }
            TerminalKeyCode::Char('q') | TerminalKeyCode::Char('Q') | TerminalKeyCode::Esc => {
                self.register_action(InspectorAction::Quit)
            }
            _ => {
                self.action_confirmation.cancel();
                false
            }
        }
    }

    fn register_action(&mut self, action: InspectorAction) -> bool {
        matches!(
            self.action_confirmation.register(action),
            ConfirmationResult::Confirmed(_)
        )
    }

    fn clear_log(&mut self) {
        self.events.clear();
        self.tracker.forget(CLEAR_LOG_KEYCODE);
        self.suppress_until_released = Some(CLEAR_LOG_KEYCODE);
    }

    fn poll(&mut self) {
        let mut values = HashMap::<KeyCode, AnalogValue>::new();
        let result = self.sdk.read_keycodes(|ctx| {
            values.extend(ctx.iter().map(|(keycode, value)| (*keycode, *value)));
        });

        match result {
            Ok(()) => {
                self.last_error = None;
                self.active_keys = collect_active_keys(&values);

                if let Some(keycode) = self.suppress_until_released
                    && !self.active_keys.iter().any(|key| key.keycode == keycode)
                {
                    self.tracker.forget(keycode);
                    self.suppress_until_released = None;
                }

                let mut events = Vec::new();
                for key in &self.active_keys {
                    if self.suppress_until_released == Some(key.keycode) {
                        continue;
                    }

                    if let Some(event) = self.tracker.update(key.keycode, key.value) {
                        events.push(event);
                    }
                }

                let released_keys = self
                    .active_keys
                    .iter()
                    .map(|key| key.keycode)
                    .collect::<std::collections::HashSet<_>>();

                for keycode in values
                    .keys()
                    .map(u16::from)
                    .filter(|keycode| !released_keys.contains(keycode))
                    .collect::<Vec<_>>()
                {
                    if self.suppress_until_released == Some(keycode) {
                        continue;
                    }

                    if let Some(event) = self.tracker.update(keycode, 0.0) {
                        events.push(event);
                    }
                }

                for event in events {
                    self.push_event(event);
                }
            }
            Err(err) => {
                self.last_error = Some(err.to_string());
                self.active_keys.clear();
            }
        }
    }

    fn push_event(&mut self, event: IntentEvent) {
        if self.events.len() == MAX_EVENTS {
            self.events.pop_front();
        }

        self.events.push_back(event);
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(10),
                Constraint::Length(3),
            ])
            .split(area);

        let key_rows = self.active_keys.iter().map(|key| {
            Row::new([
                Cell::from(format!("{}", key.keycode)),
                Cell::from(key.label),
                Cell::from(format!("{:.3}", key.value)),
                Cell::from(key.state.to_string()),
            ])
        });

        let key_table = Table::new(
            key_rows,
            [
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Length(10),
                Constraint::Min(12),
            ],
        )
        .header(
            Row::new(["Keycode", "Key", "Value", "Intent"]).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().title("Active Keys").borders(Borders::ALL));

        frame.render_widget(key_table, chunks[0]);

        let event_rows = self.events.iter().rev().map(|event| {
            Row::new([
                Cell::from(format!("{:>7.3}s", event.timestamp.as_secs_f32())),
                Cell::from(format!("{} ({})", event.label, event.keycode)),
                Cell::from(event.direction_symbol()),
                Cell::from(format!("{:.3}", event.value)),
            ])
        });

        let events = Table::new(
            event_rows,
            [
                Constraint::Length(10),
                Constraint::Length(18),
                Constraint::Length(22),
                Constraint::Min(8),
            ],
        )
        .header(
            Row::new(["Time", "Key", "Change", "Value"]).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title("Intent Events")
                .borders(Borders::ALL),
        );

        frame.render_widget(events, chunks[1]);

        let status = if let Some(action) = self.action_confirmation.pending_action() {
            Paragraph::new(Line::from(action.confirmation_text()))
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .block(Block::default().borders(Borders::ALL))
        } else if let Some(error) = &self.last_error {
            Paragraph::new(format!(
                "devices: {} | poll: {} Hz | last error: {} | W to clear log | q/Esc to quit",
                self.sdk.device_count(),
                POLL_RATE_HZ,
                error,
            ))
            .block(Block::default().borders(Borders::ALL))
        } else {
            Paragraph::new(format!(
                "devices: {} | poll: {} Hz | W to clear log | q/Esc to quit",
                self.sdk.device_count(),
                POLL_RATE_HZ,
            ))
            .block(Block::default().borders(Borders::ALL))
        };

        frame.render_widget(status, chunks[2]);
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)
}

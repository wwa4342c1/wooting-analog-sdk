use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use wooting_analog_sdk::{AnalogValue, KeyCode};

const RELEASED_THRESHOLD: f32 = 0.05;
const MEDIUM_THRESHOLD: f32 = 0.35;
const FULL_THRESHOLD: f32 = 0.80;
pub const ACTION_PRESS_WINDOW: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentState {
    Released,
    Soft,
    Medium,
    Full,
}

impl IntentState {
    pub fn classify(value: f32) -> Self {
        if value >= FULL_THRESHOLD {
            Self::Full
        } else if value >= MEDIUM_THRESHOLD {
            Self::Medium
        } else if value >= RELEASED_THRESHOLD {
            Self::Soft
        } else {
            Self::Released
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Released => 0,
            Self::Soft => 1,
            Self::Medium => 2,
            Self::Full => 3,
        }
    }
}

impl std::fmt::Display for IntentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Released => write!(f, "released"),
            Self::Soft => write!(f, "soft"),
            Self::Medium => write!(f, "medium"),
            Self::Full => write!(f, "full"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveKey {
    pub keycode: u16,
    pub label: &'static str,
    pub value: f32,
    pub state: IntentState,
}

impl ActiveKey {
    pub fn new(keycode: u16, value: f32) -> Self {
        Self {
            keycode,
            label: key_label(keycode),
            value,
            state: IntentState::classify(value),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntentEvent {
    pub keycode: u16,
    pub label: &'static str,
    pub previous: IntentState,
    pub next: IntentState,
    pub value: f32,
    pub timestamp: Duration,
}

impl IntentEvent {
    pub fn direction_symbol(&self) -> &'static str {
        if self.next.rank() > self.previous.rank() {
            "▲"
        } else {
            "▼"
        }
    }
}

#[derive(Debug)]
pub struct IntentTracker {
    started_at: Instant,
    states: HashMap<u16, IntentState>,
}

impl Default for IntentTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentTracker {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            states: HashMap::new(),
        }
    }

    pub fn update(&mut self, keycode: u16, value: f32) -> Option<IntentEvent> {
        let next = IntentState::classify(value);
        let previous = self
            .states
            .get(&keycode)
            .copied()
            .unwrap_or(IntentState::Released);

        if previous == next {
            return None;
        }

        if next == IntentState::Released {
            self.states.remove(&keycode);
        } else {
            self.states.insert(keycode, next);
        }

        Some(IntentEvent {
            keycode,
            label: key_label(keycode),
            previous,
            next,
            value,
            timestamp: self.started_at.elapsed(),
        })
    }

    pub fn forget(&mut self, keycode: u16) {
        self.states.remove(&keycode);
    }
}

pub fn collect_active_keys(values: &HashMap<KeyCode, AnalogValue>) -> Vec<ActiveKey> {
    let mut keys = values
        .iter()
        .filter_map(|(keycode, value)| {
            let keycode = u16::from(keycode);
            let value = f32::from(value);

            (IntentState::classify(value) != IntentState::Released)
                .then_some(ActiveKey::new(keycode, value))
        })
        .collect::<Vec<_>>();

    keys.sort_by_key(|key| key.keycode);
    keys
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectorAction {
    ClearLog,
    Quit,
}

impl InspectorAction {
    pub fn confirmation_text(self) -> &'static str {
        match self {
            Self::ClearLog => "W to confirm",
            Self::Quit => "q/Esc to confirm",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmationResult {
    Pending(InspectorAction),
    Confirmed(InspectorAction),
}

#[derive(Clone, Debug)]
pub struct ActionConfirmation {
    window: Duration,
    current: Option<InspectorAction>,
    count: u8,
    last_press: Option<Instant>,
}

impl Default for ActionConfirmation {
    fn default() -> Self {
        Self::new(ACTION_PRESS_WINDOW)
    }
}

impl ActionConfirmation {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            current: None,
            count: 0,
            last_press: None,
        }
    }

    pub fn pending_action(&self) -> Option<InspectorAction> {
        if self.count > 0 { self.current } else { None }
    }

    pub fn register(&mut self, action: InspectorAction) -> ConfirmationResult {
        self.register_at(action, Instant::now())
    }

    pub fn cancel(&mut self) {
        self.current = None;
        self.count = 0;
        self.last_press = None;
    }

    pub fn register_at(&mut self, action: InspectorAction, now: Instant) -> ConfirmationResult {
        let continues = self.current == Some(action)
            && self
                .last_press
                .is_some_and(|last_press| now.duration_since(last_press) <= self.window);

        if continues {
            self.count = self.count.saturating_add(1);
        } else {
            self.current = Some(action);
            self.count = 1;
        }

        self.last_press = Some(now);

        if self.count >= 2 {
            self.cancel();
            ConfirmationResult::Confirmed(action)
        } else {
            ConfirmationResult::Pending(action)
        }
    }
}

pub fn key_label(keycode: u16) -> &'static str {
    match keycode {
        0x04 => "A",
        0x05 => "B",
        0x06 => "C",
        0x07 => "D",
        0x08 => "E",
        0x09 => "F",
        0x0a => "G",
        0x0b => "H",
        0x0c => "I",
        0x0d => "J",
        0x0e => "K",
        0x0f => "L",
        0x10 => "M",
        0x11 => "N",
        0x12 => "O",
        0x13 => "P",
        0x14 => "Q",
        0x15 => "R",
        0x16 => "S",
        0x17 => "T",
        0x18 => "U",
        0x19 => "V",
        0x1a => "W",
        0x1b => "X",
        0x1c => "Y",
        0x1d => "Z",
        0x1e => "1",
        0x1f => "2",
        0x20 => "3",
        0x21 => "4",
        0x22 => "5",
        0x23 => "6",
        0x24 => "7",
        0x25 => "8",
        0x26 => "9",
        0x27 => "0",
        0x28 => "Enter",
        0x29 => "Esc",
        0x2a => "Backspace",
        0x2b => "Tab",
        0x2c => "Space",
        0x2d => "-",
        0x2e => "=",
        0x2f => "[",
        0x30 => "]",
        0x31 => "\\",
        0x33 => ";",
        0x34 => "'",
        0x35 => "`",
        0x36 => ",",
        0x37 => ".",
        0x38 => "/",
        0x39 => "Caps",
        0x3a => "F1",
        0x3b => "F2",
        0x3c => "F3",
        0x3d => "F4",
        0x3e => "F5",
        0x3f => "F6",
        0x40 => "F7",
        0x41 => "F8",
        0x42 => "F9",
        0x43 => "F10",
        0x44 => "F11",
        0x45 => "F12",
        0x46 => "Print",
        0x47 => "Scroll",
        0x48 => "Pause",
        0x49 => "Insert",
        0x4a => "Home",
        0x4b => "PageUp",
        0x4c => "Delete",
        0x4d => "End",
        0x4e => "PageDown",
        0x4f => "Right",
        0x50 => "Left",
        0x51 => "Down",
        0x52 => "Up",
        0x53 => "NumLock",
        0x54 => "Num /",
        0x55 => "Num *",
        0x56 => "Num -",
        0x57 => "Num +",
        0x58 => "Num Enter",
        0x59 => "Num 1",
        0x5a => "Num 2",
        0x5b => "Num 3",
        0x5c => "Num 4",
        0x5d => "Num 5",
        0x5e => "Num 6",
        0x5f => "Num 7",
        0x60 => "Num 8",
        0x61 => "Num 9",
        0x62 => "Num 0",
        0x63 => "Num .",
        0xe0 => "Left Ctrl",
        0xe1 => "Left Shift",
        0xe2 => "Left Alt",
        0xe3 => "Left Meta",
        0xe4 => "Right Ctrl",
        0xe5 => "Right Shift",
        0xe6 => "Right Alt",
        0xe7 => "Right Meta",
        _ => "-",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_threshold_boundaries() {
        assert_eq!(IntentState::classify(0.0), IntentState::Released);
        assert_eq!(IntentState::classify(0.049), IntentState::Released);
        assert_eq!(IntentState::classify(0.05), IntentState::Soft);
        assert_eq!(IntentState::classify(0.349), IntentState::Soft);
        assert_eq!(IntentState::classify(0.35), IntentState::Medium);
        assert_eq!(IntentState::classify(0.799), IntentState::Medium);
        assert_eq!(IntentState::classify(0.80), IntentState::Full);
        assert_eq!(IntentState::classify(1.0), IntentState::Full);
    }

    #[test]
    fn does_not_emit_when_state_is_unchanged() {
        let mut tracker = IntentTracker::new();

        assert!(tracker.update(26, 0.0).is_none());
        assert_eq!(tracker.update(26, 0.05).unwrap().next, IntentState::Soft);
        assert!(tracker.update(26, 0.10).is_none());
    }

    #[test]
    fn emits_transitions_between_intent_states() {
        let mut tracker = IntentTracker::new();

        let soft = tracker.update(26, 0.05).unwrap();
        assert_eq!(soft.previous, IntentState::Released);
        assert_eq!(soft.next, IntentState::Soft);

        let medium = tracker.update(26, 0.35).unwrap();
        assert_eq!(medium.previous, IntentState::Soft);
        assert_eq!(medium.next, IntentState::Medium);

        let full = tracker.update(26, 0.80).unwrap();
        assert_eq!(full.previous, IntentState::Medium);
        assert_eq!(full.next, IntentState::Full);
    }

    #[test]
    fn emits_transition_when_key_returns_to_released() {
        let mut tracker = IntentTracker::new();

        assert!(tracker.update(26, 0.80).is_some());

        let released = tracker.update(26, 0.0).unwrap();
        assert_eq!(released.previous, IntentState::Full);
        assert_eq!(released.next, IntentState::Released);

        assert!(tracker.update(26, 0.0).is_none());
    }

    #[test]
    fn event_direction_symbol_shows_increase_or_decrease() {
        let mut tracker = IntentTracker::new();

        let increased = tracker.update(26, 0.80).unwrap();
        assert_eq!(increased.direction_symbol(), "▲");

        let decreased = tracker.update(26, 0.10).unwrap();
        assert_eq!(decreased.direction_symbol(), "▼");
    }

    #[test]
    fn labels_known_hid_keycodes() {
        assert_eq!(key_label(0x1a), "W");
        assert_eq!(key_label(0xe1), "Left Shift");
        assert_eq!(ActiveKey::new(0x16, 1.0).label, "S");
    }

    #[test]
    fn action_confirmation_requires_two_fast_consecutive_presses() {
        let now = Instant::now();
        let mut guard = ActionConfirmation::new(Duration::from_millis(600));

        assert_eq!(
            guard.register_at(InspectorAction::ClearLog, now),
            ConfirmationResult::Pending(InspectorAction::ClearLog)
        );
        assert_eq!(
            guard.register_at(InspectorAction::ClearLog, now + Duration::from_millis(100)),
            ConfirmationResult::Confirmed(InspectorAction::ClearLog)
        );
        assert_eq!(guard.pending_action(), None);
    }

    #[test]
    fn action_confirmation_resets_when_presses_are_slow_or_different() {
        let now = Instant::now();
        let mut guard = ActionConfirmation::new(Duration::from_millis(600));

        assert_eq!(
            guard.register_at(InspectorAction::ClearLog, now),
            ConfirmationResult::Pending(InspectorAction::ClearLog)
        );
        assert_eq!(
            guard.register_at(InspectorAction::ClearLog, now + Duration::from_millis(700)),
            ConfirmationResult::Pending(InspectorAction::ClearLog)
        );
        assert_eq!(
            guard.register_at(InspectorAction::Quit, now + Duration::from_millis(800)),
            ConfirmationResult::Pending(InspectorAction::Quit)
        );
        assert_eq!(
            guard.register_at(InspectorAction::Quit, now + Duration::from_millis(900)),
            ConfirmationResult::Confirmed(InspectorAction::Quit)
        );
    }
}

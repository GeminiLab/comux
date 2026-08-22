use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Refresh,
    Up,
    Down,
    Select,
    Remove,
    RegenerateToken,
}

pub fn action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => Some(Action::Quit),
        (KeyCode::Char('r'), KeyModifiers::NONE) => Some(Action::Refresh),
        (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::Up),
        (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::Down),
        (KeyCode::Enter, _) => Some(Action::Select),
        (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Action::Remove),
        (KeyCode::Char('t'), KeyModifiers::NONE) => Some(Action::RegenerateToken),
        _ => None,
    }
}

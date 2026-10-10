//! Mapping from terminal key events to dashboard actions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A high-level operation understood by the dashboard state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Exits the dashboard.
    Quit,
    /// Refreshes management state immediately.
    Refresh,
    /// Moves selection to the previous upstream.
    Up,
    /// Moves selection to the next upstream.
    Down,
    /// Activates the selected upstream.
    Select,
    /// Deletes the selected upstream.
    Remove,
    /// Rotates the downstream client token.
    RegenerateToken,
    /// Opens the add-upstream form.
    Add,
}

/// Converts a supported key event into a dashboard action.
pub fn action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _) => Some(Action::Quit),
        (KeyCode::Char('r'), KeyModifiers::NONE) => Some(Action::Refresh),
        (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => Some(Action::Up),
        (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => Some(Action::Down),
        (KeyCode::Enter, _) => Some(Action::Select),
        (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Action::Remove),
        (KeyCode::Char('t'), KeyModifiers::NONE) => Some(Action::RegenerateToken),
        (KeyCode::Char('a'), KeyModifiers::NONE) => Some(Action::Add),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{action, Action};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn maps_a_to_add_upstream() {
        assert_eq!(
            action(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(Action::Add)
        );
    }
}

//! Mapping raw key events to high-level UI actions, per interaction mode.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Browsing the list.
    Normal,
    /// Typing into the filter box.
    Filter,
    /// Awaiting y/n on a pending deletion.
    Confirm,
    /// Help overlay shown.
    Help,
}

#[derive(Debug, Clone)]
pub enum Action {
    Quit,
    Up,
    Down,
    Top,
    Bottom,
    ToggleSelect,
    SelectAll,
    ClearSelection,
    RequestDelete,
    ConfirmYes,
    ConfirmNo,
    CycleSort,
    StartFilter,
    FilterInput(char),
    FilterBackspace,
    FilterDone,
    FilterClear,
    ToggleHelp,
    Nothing,
}

/// Translate a key press into an [`Action`] given the current [`Mode`].
pub fn map(mode: Mode, key: KeyEvent) -> Action {
    match mode {
        Mode::Confirm => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Action::ConfirmYes,
            _ => Action::ConfirmNo,
        },
        Mode::Help => match key.code {
            KeyCode::Char('q') => Action::Quit,
            _ => Action::ToggleHelp,
        },
        Mode::Filter => match key.code {
            KeyCode::Esc => Action::FilterClear,
            KeyCode::Enter => Action::FilterDone,
            KeyCode::Backspace => Action::FilterBackspace,
            KeyCode::Char(c) => Action::FilterInput(c),
            _ => Action::Nothing,
        },
        Mode::Normal => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                return Action::Quit;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
                KeyCode::Up | KeyCode::Char('k') => Action::Up,
                KeyCode::Down | KeyCode::Char('j') => Action::Down,
                KeyCode::Char('g') | KeyCode::Home => Action::Top,
                KeyCode::Char('G') | KeyCode::End => Action::Bottom,
                KeyCode::Char(' ') => Action::ToggleSelect,
                KeyCode::Char('a') => Action::SelectAll,
                KeyCode::Char('c') => Action::ClearSelection,
                KeyCode::Char('d') | KeyCode::Delete | KeyCode::Enter => Action::RequestDelete,
                KeyCode::Char('s') => Action::CycleSort,
                KeyCode::Char('/') => Action::StartFilter,
                KeyCode::Char('?') => Action::ToggleHelp,
                _ => Action::Nothing,
            }
        }
    }
}

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::ui::InputMode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Quit,

    // Navigation
    MoveUp,
    MoveDown,
    FocusWorkspaces,
    FocusTasks,
    ToggleFocus,

    // Actions
    Add,
    AddSubtask,
    ToggleComplete,
    Edit,
    StartSearch,
    ClearSearch,
    ToggleSort,
    ToggleDates,
    ArchiveCompleted,
    Delete,
    ShowHelp,
    HideHelp,

    // Edit popup
    SaveEdit,
    CancelEdit,
    NextEditField,
    SelectTitleField,
    SelectDueDateField,

    // Create popup
    ConfirmCreate,
    CancelCreate,

    // Search prompt
    ConfirmSearch,
    CancelSearch,

    // Delete confirmation
    ConfirmDelete,
    CancelDelete,

    // Raw key forwarded to the active text input
    TextInput(KeyEvent),
}

impl Command {
    pub fn allowed_in(&self, mode: &InputMode) -> bool {
        match self {
            Self::SaveEdit
            | Self::CancelEdit
            | Self::NextEditField
            | Self::SelectTitleField
            | Self::SelectDueDateField => *mode == InputMode::Insert,
            Self::ConfirmCreate | Self::CancelCreate => *mode == InputMode::Creating,
            Self::ConfirmSearch | Self::CancelSearch => *mode == InputMode::Search,
            Self::ConfirmDelete | Self::CancelDelete => *mode == InputMode::DeleteConfirm,
            Self::HideHelp => *mode == InputMode::Help,
            Self::TextInput(_) => matches!(
                mode,
                InputMode::Insert | InputMode::Creating | InputMode::Search
            ),
            _ => *mode == InputMode::Normal,
        }
    }

    pub fn from_key(key: KeyEvent, mode: &InputMode) -> Option<Self> {
        let command = match mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('q') => Self::Quit,
                KeyCode::Down | KeyCode::Char('j') => Self::MoveDown,
                KeyCode::Up | KeyCode::Char('k') => Self::MoveUp,
                KeyCode::Right | KeyCode::Char('l') => Self::FocusTasks,
                KeyCode::Left | KeyCode::Char('h') => Self::FocusWorkspaces,
                KeyCode::Tab => Self::ToggleFocus,
                KeyCode::Esc => Self::ClearSearch,
                KeyCode::Char('A') => Self::AddSubtask,
                KeyCode::Char('a') => Self::Add,
                KeyCode::Char('c') | KeyCode::Char(' ') => Self::ToggleComplete,
                KeyCode::Char('e') => Self::Edit,
                KeyCode::Char('/') => Self::StartSearch,
                KeyCode::Char('s') => Self::ToggleSort,
                KeyCode::Char('d') => Self::ToggleDates,
                KeyCode::Char('x') => Self::ArchiveCompleted,
                KeyCode::Char('D') => Self::Delete,
                KeyCode::Char('?') => Self::ShowHelp,
                _ => return None,
            },
            InputMode::Insert => match key.code {
                KeyCode::Enter => Self::SaveEdit,
                KeyCode::Esc => Self::CancelEdit,
                KeyCode::Tab => Self::NextEditField,
                KeyCode::Up => Self::SelectTitleField,
                KeyCode::Down => Self::SelectDueDateField,
                _ => Self::TextInput(key),
            },
            InputMode::Creating => match key.code {
                KeyCode::Enter => Self::ConfirmCreate,
                KeyCode::Esc => Self::CancelCreate,
                _ => Self::TextInput(key),
            },
            InputMode::Search => match key.code {
                KeyCode::Enter => Self::ConfirmSearch,
                KeyCode::Esc => Self::CancelSearch,
                _ => Self::TextInput(key),
            },
            InputMode::DeleteConfirm => match key.code {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => Self::ConfirmDelete,
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Self::CancelDelete,
                _ => return None,
            },
            InputMode::Help => match key.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => Self::HideHelp,
                _ => return None,
            },
        };
        Some(command)
    }
}

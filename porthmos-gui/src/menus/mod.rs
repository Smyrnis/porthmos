use iced::Point;

use crate::app::PanelSide;
use crate::shortcuts::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoreItem {
    Palette,
    SelectAll,
    ToggleHidden,
    Preview,
    Edit,
    Search,
    Bookmarks,
    BookmarkHere,
    History,
    Sync,
    Terminal,
    CheckForUpdates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoreState {
    pub hidden_shown: bool,
    pub file: bool,
    pub terminal: bool,
    pub sync: bool,
}

pub fn more_items(state: MoreState) -> Vec<(MoreItem, String, bool)> {
    let hidden = if state.hidden_shown { "Hide hidden files" } else { "Show hidden files" };
    vec![
        (MoreItem::Palette, "Command palette…".to_string(), true),
        (MoreItem::SelectAll, "Select all".to_string(), true),
        (MoreItem::ToggleHidden, hidden.to_string(), true),
        (MoreItem::Preview, "Preview".to_string(), state.file),
        (MoreItem::Edit, "Edit".to_string(), state.file),
        (MoreItem::Search, "Search…".to_string(), true),
        (MoreItem::Bookmarks, "Bookmarks…".to_string(), true),
        (MoreItem::BookmarkHere, "Bookmark this folder".to_string(), true),
        (MoreItem::History, "Transfer history…".to_string(), true),
        (MoreItem::Sync, "Synchronize…".to_string(), state.sync),
        (MoreItem::Terminal, "Open terminal here".to_string(), state.terminal),
        (MoreItem::CheckForUpdates, "Check for updates".to_string(), true),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Open,
    Preview,
    Edit,
    CopyToOther,
    MoveToOther,
    Rename,
    Delete,
    NewFolder,
    Refresh,
}

impl MenuItem {
    pub fn title(self) -> &'static str {
        match self {
            MenuItem::Open => "Open",
            MenuItem::Preview => "Preview",
            MenuItem::Edit => "Edit",
            MenuItem::CopyToOther => "Copy to other panel",
            MenuItem::MoveToOther => "Move to other panel",
            MenuItem::Rename => "Rename…",
            MenuItem::Delete => "Delete",
            MenuItem::NewFolder => "New folder",
            MenuItem::Refresh => "Refresh",
        }
    }

    pub fn action(self) -> Action {
        match self {
            MenuItem::Open => Action::Open,
            MenuItem::Preview => Action::Preview,
            MenuItem::Edit => Action::Edit,
            MenuItem::CopyToOther => Action::Copy,
            MenuItem::MoveToOther => Action::Move,
            MenuItem::Rename => Action::Rename,
            MenuItem::Delete => Action::Delete,
            MenuItem::NewFolder => Action::NewFolder,
            MenuItem::Refresh => Action::Refresh,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    Entries { count: usize, only_parent: bool, file: bool },
    Background,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenu {
    pub side: PanelSide,
    pub target: MenuTarget,
    pub at: Point,
}

pub fn context_items(target: &MenuTarget, other_panel: bool) -> Vec<(MenuItem, bool)> {
    match target {
        MenuTarget::Background => vec![(MenuItem::NewFolder, true), (MenuItem::Refresh, true)],
        MenuTarget::Entries { count, only_parent, file } => {
            let real = !only_parent;
            vec![
                (MenuItem::Open, *count == 1),
                (MenuItem::Preview, *file && real),
                (MenuItem::Edit, *file && real),
                (MenuItem::CopyToOther, other_panel && real),
                (MenuItem::MoveToOther, other_panel && real),
                (MenuItem::Rename, *count == 1 && real),
                (MenuItem::Delete, real),
                (MenuItem::NewFolder, true),
                (MenuItem::Refresh, true),
            ]
        }
    }
}

#[cfg(test)]
mod tests;

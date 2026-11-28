use iced::Point;
use porthmos_core::i18n::label as tr;

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
    Reopen,
    CheckForUpdates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoreState {
    pub hidden_shown: bool,
    pub file: bool,
    pub sync: bool,
    pub reopen: bool,
}

pub fn more_items(state: MoreState) -> Vec<(MoreItem, String, bool)> {
    let hidden = if state.hidden_shown { tr("Hide hidden files") } else { tr("Show hidden files") };
    vec![
        (MoreItem::Palette, tr("Command palette…"), true),
        (MoreItem::SelectAll, tr("Select all"), true),
        (MoreItem::ToggleHidden, hidden.to_string(), true),
        (MoreItem::Preview, tr("Preview"), state.file),
        (MoreItem::Edit, tr("Edit"), state.file),
        (MoreItem::Search, tr("Search…"), true),
        (MoreItem::Bookmarks, tr("Bookmarks…"), true),
        (MoreItem::BookmarkHere, tr("Bookmark this folder"), true),
        (MoreItem::History, tr("Transfer history…"), true),
        (MoreItem::Sync, tr("Synchronize…"), state.sync),
        (MoreItem::Reopen, tr("Reopen last connections"), state.reopen),
        (MoreItem::CheckForUpdates, tr("Check for updates"), true),
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
    Permissions,
    Properties,
    FollowAsLog,
    Delete,
    NewFolder,
    Refresh,
}

impl MenuItem {
    pub fn title(self) -> String {
        match self {
            MenuItem::Open => tr("Open"),
            MenuItem::Preview => tr("Preview"),
            MenuItem::Edit => tr("Edit"),
            MenuItem::CopyToOther => tr("Copy to other panel"),
            MenuItem::MoveToOther => tr("Move to other panel"),
            MenuItem::Rename => tr("Rename…"),
            MenuItem::Permissions => tr("Permissions…"),
            MenuItem::Properties => tr("Properties…"),
            MenuItem::FollowAsLog => tr("Follow as log"),
            MenuItem::Delete => tr("Delete"),
            MenuItem::NewFolder => tr("New folder"),
            MenuItem::Refresh => tr("Refresh"),
        }
    }

    pub fn action(self) -> Option<Action> {
        Some(match self {
            MenuItem::Open => Action::Open,
            MenuItem::Preview => Action::Preview,
            MenuItem::Edit => Action::Edit,
            MenuItem::CopyToOther => Action::Copy,
            MenuItem::MoveToOther => Action::Move,
            MenuItem::Rename => Action::Rename,
            MenuItem::Permissions => Action::Permissions,
            MenuItem::Properties => Action::Properties,
            MenuItem::FollowAsLog => return None,
            MenuItem::Delete => Action::Delete,
            MenuItem::NewFolder => Action::NewFolder,
            MenuItem::Refresh => Action::Refresh,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    Entries { count: usize, only_parent: bool, file: bool, follow_log: bool },
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
        MenuTarget::Entries { count, only_parent, file, follow_log } => {
            let real = !only_parent;
            let mut items = vec![
                (MenuItem::Open, *count == 1),
                (MenuItem::Preview, *file && real),
                (MenuItem::Edit, *file && real),
                (MenuItem::CopyToOther, other_panel && real),
                (MenuItem::MoveToOther, other_panel && real),
                (MenuItem::Rename, *count == 1 && real),
                (MenuItem::Permissions, real),
                (MenuItem::Properties, real),
                (MenuItem::Delete, real),
                (MenuItem::NewFolder, true),
                (MenuItem::Refresh, true),
            ];
            if *follow_log && let Some(edit) = items.iter().position(|(item, _)| *item == MenuItem::Edit) {
                items.insert(edit + 1, (MenuItem::FollowAsLog, true));
            }
            items
        }
    }
}

#[cfg(test)]
mod tests;

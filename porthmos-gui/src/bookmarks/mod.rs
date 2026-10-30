use porthmos_core::config::bookmarks::Bookmark;
use porthmos_core::{SessionId, VfsPath};

use crate::sessions::Sessions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goto {
    Local(VfsPath),
    Session { id: SessionId, path: VfsPath },
    NotConnected(String),
}

#[derive(Debug)]
pub struct BookmarksState {
    items: Vec<Bookmark>,
    selected: Option<usize>,
}

pub fn default_label(path: &VfsPath) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "bookmark".to_string())
}

pub fn row_text(bookmark: &Bookmark) -> (String, String) {
    let place = match &bookmark.host {
        Some(host) => format!("{} · {host}", bookmark.path),
        None => bookmark.path.to_string(),
    };
    (bookmark.label.clone(), place)
}

pub fn goto(bookmark: &Bookmark, sessions: &Sessions) -> Goto {
    match &bookmark.host {
        None => Goto::Local(bookmark.path.clone()),
        Some(host) => match sessions.by_name(host) {
            Some(session) => Goto::Session { id: session.id, path: bookmark.path.clone() },
            None => Goto::NotConnected(host.clone()),
        },
    }
}

impl BookmarksState {
    pub fn new(items: Vec<Bookmark>) -> Self {
        let selected = (!items.is_empty()).then_some(0);
        Self { items, selected }
    }

    pub fn replace(&mut self, items: Vec<Bookmark>) {
        self.selected = match (self.selected, items.len()) {
            (_, 0) => None,
            (Some(row), count) => Some(row.min(count - 1)),
            (None, _) => Some(0),
        };
        self.items = items;
    }

    pub fn items(&self) -> &[Bookmark] {
        &self.items
    }

    pub fn select(&mut self, row: usize) {
        if row < self.items.len() {
            self.selected = Some(row);
        }
    }

    pub fn move_selection(&mut self, down: bool) {
        let last = self.items.len().checked_sub(1);
        self.selected = match (self.selected, last) {
            (_, None) => None,
            (None, Some(_)) => Some(0),
            (Some(current), Some(last)) if down => Some((current + 1).min(last)),
            (Some(current), Some(_)) => Some(current.saturating_sub(1)),
        };
    }

    pub fn selected(&self) -> Option<(usize, &Bookmark)> {
        let row = self.selected?;
        self.items.get(row).map(|bookmark| (row, bookmark))
    }
}

#[cfg(test)]
mod tests;

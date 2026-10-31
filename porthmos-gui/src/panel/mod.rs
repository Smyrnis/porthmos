use std::collections::HashSet;

use porthmos_core::listing::{Listing, Row, SortKey, SortOrder, SortSpec};
use porthmos_core::{Entry, VfsPath};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RowKey {
    Parent,
    Name(String),
}

#[derive(Debug, Clone)]
pub struct Panel {
    listing: Listing,
    cursor: usize,
    anchor: usize,
    selection: HashSet<VfsPath>,
    free_space: Option<(VfsPath, Option<u64>)>,
}

impl Panel {
    pub fn new(path: VfsPath, sort_spec: SortSpec, show_hidden: bool) -> Self {
        Self {
            listing: Listing::new(path, Vec::new(), sort_spec, show_hidden),
            cursor: 0,
            anchor: 0,
            selection: HashSet::new(),
            free_space: None,
        }
    }

    pub fn path(&self) -> &VfsPath {
        self.listing.path()
    }

    pub fn rows(&self) -> &[Row] {
        self.listing.rows()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn sort_spec(&self) -> SortSpec {
        self.listing.sort_spec()
    }

    pub fn show_hidden(&self) -> bool {
        self.listing.show_hidden()
    }

    pub fn filter(&self) -> Option<&str> {
        self.listing.filter()
    }

    pub fn match_count(&self) -> (usize, usize) {
        self.listing.match_count()
    }

    pub fn selection_count(&self) -> usize {
        self.selection.len()
    }

    pub fn selected(&self) -> Vec<Entry> {
        self.rows()
            .iter()
            .filter_map(|row| match row {
                Row::Entry(entry) if self.selection.contains(&entry.path) => Some(entry.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn selected_paths(&self) -> &HashSet<VfsPath> {
        &self.selection
    }

    pub fn target_entries(&self) -> Vec<Entry> {
        let selected = self.selected();
        if !selected.is_empty() {
            return selected;
        }
        match self.rows().get(self.cursor) {
            Some(Row::Entry(entry)) => vec![entry.clone()],
            _ => Vec::new(),
        }
    }

    pub fn cursor_entry(&self) -> Option<Entry> {
        match self.rows().get(self.cursor) {
            Some(Row::Entry(entry)) => Some(entry.clone()),
            _ => None,
        }
    }

    pub fn has_entry_named(&self, name: &str) -> bool {
        self.rows().iter().any(|row| matches!(row, Row::Entry(entry) if entry.name == name))
    }

    pub fn free_bytes(&self) -> Option<u64> {
        match &self.free_space {
            Some((path, bytes)) if path == self.listing.path() => *bytes,
            _ => None,
        }
    }

    pub fn set_free_space(&mut self, path: VfsPath, bytes: Option<u64>) {
        self.free_space = Some((path, bytes));
    }

    pub fn replace_listing(&mut self, path: VfsPath, entries: Vec<Entry>) {
        if &path == self.listing.path() {
            self.reshape(|listing| listing.replace(path, entries));
        } else {
            self.listing.replace(path, entries);
            self.cursor = 0;
            self.anchor = 0;
            self.selection.clear();
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.reshape(Listing::toggle_hidden);
    }

    pub fn cycle_sort(&mut self) {
        self.reshape(Listing::cycle_sort);
    }

    pub fn sort_by(&mut self, key: SortKey) {
        let current = self.listing.sort_spec();
        let order = match (current.key == key, current.order) {
            (true, SortOrder::Ascending) => SortOrder::Descending,
            _ => SortOrder::Ascending,
        };
        self.reshape(|listing| listing.set_sort_spec(SortSpec { key, order }));
    }

    pub fn set_filter(&mut self, filter: Option<&str>) {
        self.reshape(|listing| listing.set_filter(filter));
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let target = self.cursor.saturating_add_signed(delta);
        self.set_cursor(target);
    }

    pub fn focus_name(&mut self, name: &str) -> bool {
        let position = self.rows().iter().position(|row| matches!(row, Row::Entry(entry) if entry.name == name));
        match position {
            Some(row) => {
                self.set_cursor(row);
                true
            }
            None => false,
        }
    }

    pub fn set_cursor(&mut self, row: usize) {
        self.cursor = row.min(self.last_row());
    }

    pub fn click(&mut self, row: usize, modifiers: Modifiers) {
        let Some(clicked) = self.rows().get(row) else {
            return;
        };
        let clicked_path = match clicked {
            Row::Entry(entry) => Some(entry.path.clone()),
            Row::Parent => None,
        };
        if modifiers.shift {
            self.select_range(self.anchor, row);
            self.cursor = row;
            return;
        }
        if !modifiers.ctrl {
            self.selection.clear();
        }
        if let Some(path) = clicked_path {
            let toggled_off = modifiers.ctrl && self.selection.remove(&path);
            if !toggled_off {
                self.selection.insert(path);
            }
        }
        self.cursor = row;
        self.anchor = row;
    }

    pub fn select_all(&mut self) {
        self.selection = self
            .rows()
            .iter()
            .filter_map(|row| match row {
                Row::Entry(entry) => Some(entry.path.clone()),
                Row::Parent => None,
            })
            .collect();
    }

    pub fn open_target(&self, row: usize) -> Option<VfsPath> {
        match self.rows().get(row)? {
            Row::Parent => self.listing.path().parent(),
            Row::Entry(entry) if entry.is_dir => Some(entry.path.clone()),
            Row::Entry(_) => None,
        }
    }

    fn last_row(&self) -> usize {
        self.rows().len().saturating_sub(1)
    }

    fn select_range(&mut self, from: usize, to: usize) {
        let (first, last) = (from.min(to), from.max(to));
        let range: HashSet<VfsPath> = self
            .rows()
            .iter()
            .enumerate()
            .filter(|(index, _)| (first..=last).contains(index))
            .filter_map(|(_, row)| match row {
                Row::Entry(entry) => Some(entry.path.clone()),
                Row::Parent => None,
            })
            .collect();
        self.selection = range;
    }

    fn key_at(&self, index: usize) -> Option<RowKey> {
        match self.rows().get(index)? {
            Row::Parent => Some(RowKey::Parent),
            Row::Entry(entry) => Some(RowKey::Name(entry.name.clone())),
        }
    }

    fn index_of(&self, key: &RowKey) -> Option<usize> {
        self.rows().iter().position(|row| match (row, key) {
            (Row::Parent, RowKey::Parent) => true,
            (Row::Entry(entry), RowKey::Name(name)) => &entry.name == name,
            _ => false,
        })
    }

    fn reshape(&mut self, change: impl FnOnce(&mut Listing)) {
        let cursor_key = self.key_at(self.cursor);
        let anchor_key = self.key_at(self.anchor);
        change(&mut self.listing);
        let relocated = |panel: &Panel, key: Option<RowKey>, fallback: usize| {
            key.and_then(|key| panel.index_of(&key)).unwrap_or(fallback)
        };
        self.cursor = relocated(self, cursor_key, self.cursor).min(self.last_row());
        self.anchor = relocated(self, anchor_key, self.cursor).min(self.last_row());
        self.prune_selection();
    }

    fn prune_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let visible: HashSet<&VfsPath> = self
            .listing
            .rows()
            .iter()
            .filter_map(|row| match row {
                Row::Entry(entry) => Some(&entry.path),
                Row::Parent => None,
            })
            .collect();
        self.selection.retain(|path| visible.contains(path));
    }
}

#[cfg(test)]
mod tests;

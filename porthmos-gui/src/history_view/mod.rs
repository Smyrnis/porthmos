use chrono::TimeZone;
use porthmos_core::history::{self, HistoryEntry};

use crate::view::format::size;

#[derive(Debug)]
pub struct HistoryState {
    entries: Vec<HistoryEntry>,
    filter: String,
    selected: Option<usize>,
}

pub fn printable(text: &str) -> String {
    text.chars().map(|character| if character.is_control() { '?' } else { character }).collect()
}

pub fn row_text(entry: &HistoryEntry) -> (String, String) {
    let summary = format!(
        "{} · {} · {} · {}/{} files · {}",
        printable(&entry.connection),
        history::direction_text(entry.direction),
        entry.result.text(),
        entry.files_done,
        entry.files_total,
        size(entry.bytes, false),
    );
    (printable(&entry.label), summary)
}

pub fn details<Tz: TimeZone>(entry: &HistoryEntry, zone: &Tz) -> (String, String)
where
    Tz::Offset: std::fmt::Display,
{
    let finished = entry.finished_at.with_timezone(zone).format("%Y-%m-%d %H:%M:%S").to_string();
    history::details(entry, &finished, &size(entry.bytes, false))
}

impl HistoryState {
    pub fn new(entries: Vec<HistoryEntry>) -> Self {
        let mut state = Self { entries, filter: String::new(), selected: None };
        state.settle_selection();
        state
    }

    fn settle_selection(&mut self) {
        let count = self.visible().len();
        self.selected = match (self.selected, count) {
            (_, 0) => None,
            (Some(row), count) => Some(row.min(count - 1)),
            (None, _) => Some(0),
        };
    }

    pub fn replace(&mut self, entries: Vec<HistoryEntry>) {
        self.entries = entries;
        self.settle_selection();
    }

    pub fn set_filter(&mut self, text: String) {
        self.filter = text;
        self.settle_selection();
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn total(&self) -> usize {
        self.entries.len()
    }

    pub fn visible(&self) -> Vec<&HistoryEntry> {
        self.entries.iter().filter(|entry| history::matches(entry, &self.filter)).collect()
    }

    pub fn select(&mut self, row: usize) {
        if row < self.visible().len() {
            self.selected = Some(row);
        }
    }

    pub fn move_selection(&mut self, down: bool) {
        let last = self.visible().len().checked_sub(1);
        self.selected = match (self.selected, last) {
            (_, None) => None,
            (None, Some(_)) => Some(0),
            (Some(current), Some(last)) if down => Some((current + 1).min(last)),
            (Some(current), Some(_)) => Some(current.saturating_sub(1)),
        };
    }

    pub fn selected_row(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected(&self) -> Option<&HistoryEntry> {
        let row = self.selected?;
        self.visible().get(row).copied()
    }
}

#[cfg(test)]
mod tests;

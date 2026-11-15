use chrono::TimeZone;
use porthmos_core::history::{self, HistoryEntry, HistoryResult};
use porthmos_core::transfer::Direction;

use crate::view::format::size;

const DETAIL_NAMES: usize = 10;

#[derive(Debug)]
pub struct HistoryState {
    entries: Vec<HistoryEntry>,
    filter: String,
    selected: Option<usize>,
}

pub fn printable(text: &str) -> String {
    text.chars().map(|character| if character.is_control() { '?' } else { character }).collect()
}

fn direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::Upload => "Upload",
        Direction::Download => "Download",
    }
}

pub fn row_text(entry: &HistoryEntry) -> (String, String) {
    let summary = format!(
        "{} · {} · {} · {}/{} files · {}",
        printable(&entry.connection),
        direction_text(entry.direction),
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
    let finished = entry.finished_at.with_timezone(zone).format("%Y-%m-%d %H:%M:%S");
    let result = match &entry.result {
        HistoryResult::PartlyFailed { failed } => format!("partly failed ({failed} failed)"),
        other => other.text().to_string(),
    };
    let mut lines = vec![
        format!("Finished:   {finished}"),
        format!("Connection: {} ({})", printable(&entry.connection), direction_text(entry.direction)),
        format!(
            "Result:     {result}, {} of {} files, {}",
            entry.files_done,
            entry.files_total,
            size(entry.bytes, false)
        ),
    ];
    if !entry.local_path.is_empty() {
        lines.push(format!("Local:      {}", printable(&entry.local_path)));
    }
    if !entry.remote_path.is_empty() {
        lines.push(format!("Remote:     {}", printable(&entry.remote_path)));
    }
    if !entry.failed_files.is_empty() {
        lines.push("Failed files:".to_string());
        lines.extend(entry.failed_files.iter().take(DETAIL_NAMES).map(|name| format!("- {}", printable(name))));
        let shown = entry.failed_files.len().min(DETAIL_NAMES);
        let hidden = entry.failed_count.max(entry.failed_files.len()).saturating_sub(shown);
        if hidden > 0 {
            lines.push(format!("and {hidden} more…"));
        }
    }
    let failed = matches!(entry.result, HistoryResult::Failed | HistoryResult::PartlyFailed { .. });
    if failed || !entry.failed_files.is_empty() {
        lines.push("The reasons are in porthmos.log.".to_string());
    }
    (format!("Transfer: {}", printable(&entry.label)), lines.join("\n"))
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

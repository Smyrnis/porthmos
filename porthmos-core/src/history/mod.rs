use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use porthmos_vfs::glob_match;
use serde::{Deserialize, Serialize};

use crate::{
    Paths, i18n,
    persist::{Loaded, read_or_set_aside, toml_problem},
    transfer::Direction,
};

pub const MAX_ENTRIES: usize = 1000;
pub const MAX_FAILED_FILES: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryResult {
    Done,
    PartlyFailed { failed: usize },
    Failed,
    Cancelled,
    Interrupted,
}

impl HistoryResult {
    pub fn text(&self) -> String {
        let key = match self {
            HistoryResult::Done => "history.result.done",
            HistoryResult::PartlyFailed { .. } => "history.result.partly_failed",
            HistoryResult::Failed => "history.result.failed",
            HistoryResult::Cancelled => "history.result.cancelled",
            HistoryResult::Interrupted => "history.result.interrupted",
        };
        i18n::text(key)
    }

    pub fn detailed_text(&self) -> String {
        match self {
            HistoryResult::PartlyFailed { failed } => {
                i18n::text_with("history.result.partly_failed_count", &[("failed", &failed.to_string())])
            }
            other => other.text(),
        }
    }
}

pub fn direction_text(direction: Direction) -> String {
    match direction {
        Direction::Upload => i18n::text("history.direction.upload"),
        Direction::Download => i18n::text("history.direction.download"),
    }
}

const DETAIL_NAMES: usize = 10;

pub fn details(entry: &HistoryEntry, finished: &str, size: &str) -> (String, String) {
    let printable = crate::edit::printable;
    let mut lines = vec![
        i18n::text_with("history.details.finished", &[("value", finished)]),
        i18n::text_with(
            "history.details.connection",
            &[("name", &printable(&entry.connection)), ("direction", &direction_text(entry.direction))],
        ),
        i18n::text_with(
            "history.details.result",
            &[
                ("result", &entry.result.detailed_text()),
                ("done", &entry.files_done.to_string()),
                ("total", &entry.files_total.to_string()),
                ("size", size),
            ],
        ),
    ];
    if !entry.local_path.is_empty() {
        lines.push(i18n::text_with("history.details.local", &[("path", &printable(&entry.local_path))]));
    }
    if !entry.remote_path.is_empty() {
        lines.push(i18n::text_with("history.details.remote", &[("path", &printable(&entry.remote_path))]));
    }
    if !entry.failed_files.is_empty() {
        lines.push(i18n::text("history.details.failed_files"));
        lines.extend(entry.failed_files.iter().take(DETAIL_NAMES).map(|name| format!("- {}", printable(name))));
        let shown = entry.failed_files.len().min(DETAIL_NAMES);
        let hidden = entry.failed_count.max(entry.failed_files.len()).saturating_sub(shown);
        if hidden > 0 {
            lines.push(i18n::text_with("history.details.more", &[("count", &hidden.to_string())]));
        }
    }
    let failed = matches!(entry.result, HistoryResult::Failed | HistoryResult::PartlyFailed { .. });
    if failed || !entry.failed_files.is_empty() {
        lines.push(i18n::text("history.details.log"));
    }
    (i18n::text_with("history.details.title", &[("label", &printable(&entry.label))]), lines.join("\n"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub finished_at: DateTime<Utc>,
    pub connection: String,
    pub direction: Direction,
    pub label: String,
    pub local_path: String,
    pub remote_path: String,
    pub files_done: usize,
    pub files_total: usize,
    pub bytes: u64,
    pub result: HistoryResult,
    #[serde(default)]
    pub failed_count: usize,
    #[serde(default)]
    pub failed_files: Vec<String>,
}

#[derive(Deserialize)]
struct HistoryFile {
    entries: Vec<HistoryEntry>,
}

#[derive(Serialize)]
struct HistoryFileRef<'a> {
    entries: &'a [HistoryEntry],
}

pub struct History {
    path: PathBuf,
    entries: Arc<Vec<HistoryEntry>>,
    writable: bool,
}

impl History {
    pub fn load(paths: &Paths) -> (Self, Option<String>) {
        let path = paths.history_file();
        let mut history = Self { path: path.clone(), entries: Arc::new(Vec::new()), writable: true };
        match read_or_set_aside(&path, "transfer history", |text| {
            toml::from_str::<HistoryFile>(text).map_err(|err| toml_problem(text, &err))
        }) {
            Loaded::Missing => (history, None),
            Loaded::Ready(file) => {
                history.entries = Arc::new(file.entries);
                history.trim();
                (history, None)
            }
            Loaded::SetAside { warning, protected } => {
                history.writable = !protected;
                (history, Some(warning))
            }
        }
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn newest_first(&self) -> Vec<HistoryEntry> {
        self.entries.iter().rev().cloned().collect()
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        Arc::make_mut(&mut self.entries).push(entry);
        self.trim();
    }

    pub fn render(&self) -> Result<String> {
        render_entries(&self.entries)
    }

    pub fn snapshot(&self) -> Arc<Vec<HistoryEntry>> {
        self.entries.clone()
    }

    pub fn clear_memory(&mut self) {
        self.entries = Arc::new(Vec::new());
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    #[cfg(test)]
    pub(crate) fn set_writable_for_test(&mut self, writable: bool) {
        self.writable = writable;
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn trim(&mut self) {
        let excess = self.entries.len().saturating_sub(MAX_ENTRIES);
        if excess > 0 {
            Arc::make_mut(&mut self.entries).drain(..excess);
        }
    }
}

pub fn render_entries(entries: &[HistoryEntry]) -> Result<String> {
    Ok(toml::to_string(&HistoryFileRef { entries })?)
}

pub fn matches(entry: &HistoryEntry, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    let result = entry.result.text();
    let fields = [
        entry.connection.as_str(),
        entry.label.as_str(),
        entry.local_path.as_str(),
        entry.remote_path.as_str(),
        result.as_str(),
    ];
    if filter.contains(['*', '?']) {
        fields.iter().any(|field| glob_match(filter, field))
    } else {
        let needle = filter.to_lowercase();
        fields.iter().any(|field| field.to_lowercase().contains(&needle))
    }
}

#[cfg(test)]
pub(crate) mod testing;

#[cfg(test)]
mod tests;

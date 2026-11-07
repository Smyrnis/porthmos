use std::sync::Arc;

use porthmos_core::preview::Preview;
use porthmos_core::{Entry, Location, VfsPath};

use crate::preview_view::PreviewState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchStatus {
    Idle,
    Searching,
    Done { truncated: bool },
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    pub location: Location,
    pub root: VfsPath,
    pub pattern: String,
}

#[derive(Debug)]
pub enum SearchPreview {
    Folder(String),
    File(PreviewState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSync {
    Keep,
    Cancel,
    Request { path: VfsPath },
}

#[derive(Debug)]
pub struct SearchState {
    location: Location,
    root: VfsPath,
    pattern: String,
    results: Vec<Entry>,
    selected: Option<usize>,
    status: SearchStatus,
    preview_enabled: bool,
    preview: Option<SearchPreview>,
    previewed: Option<VfsPath>,
}

pub fn result_parent(entry: &Entry) -> VfsPath {
    entry.path.parent().unwrap_or_else(|| entry.path.clone())
}

impl SearchState {
    pub fn new(location: Location, root: VfsPath) -> Self {
        Self {
            location,
            root,
            pattern: String::new(),
            results: Vec::new(),
            selected: None,
            status: SearchStatus::Idle,
            preview_enabled: false,
            preview: None,
            previewed: None,
        }
    }

    pub fn with_preview(mut self, enabled: bool) -> Self {
        self.preview_enabled = enabled;
        self
    }

    pub fn preview_enabled(&self) -> bool {
        self.preview_enabled
    }

    pub fn set_preview_enabled(&mut self, enabled: bool) {
        self.preview_enabled = enabled;
        self.reset_preview();
    }

    pub fn reset_preview(&mut self) {
        self.preview = None;
        self.previewed = None;
    }

    pub fn preview(&self) -> Option<&SearchPreview> {
        self.preview.as_ref()
    }

    pub fn sync_preview(&mut self, request_id: u64) -> PreviewSync {
        if !self.preview_enabled {
            return PreviewSync::Keep;
        }
        let Some(entry) = self.selected.and_then(|row| self.results.get(row)) else {
            self.previewed = None;
            return if self.preview.take().is_some() { PreviewSync::Cancel } else { PreviewSync::Keep };
        };
        if self.previewed.as_ref() == Some(&entry.path) {
            return PreviewSync::Keep;
        }
        self.previewed = Some(entry.path.clone());
        if entry.is_dir {
            self.preview = Some(SearchPreview::Folder(entry.name.clone()));
            return PreviewSync::Cancel;
        }
        self.preview = Some(SearchPreview::File(PreviewState::loading(request_id, entry.name.clone())));
        PreviewSync::Request { path: entry.path.clone() }
    }

    pub fn preview_arrived(&mut self, request_id: u64, result: Result<Arc<Preview>, String>) {
        if let Some(SearchPreview::File(state)) = self.preview.as_mut() {
            state.finish(request_id, result);
        }
    }

    pub fn location(&self) -> Location {
        self.location
    }

    #[cfg(test)]
    pub fn root(&self) -> &VfsPath {
        &self.root
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    pub fn results(&self) -> &[Entry] {
        &self.results
    }

    pub fn status(&self) -> &SearchStatus {
        &self.status
    }

    pub fn set_pattern(&mut self, text: String) -> Option<SearchRequest> {
        self.results.clear();
        self.selected = None;
        self.preview = None;
        self.previewed = None;
        let blank = text.trim().is_empty();
        let request = (!blank).then(|| SearchRequest {
            location: self.location,
            root: self.root.clone(),
            pattern: format!("*{text}*"),
        });
        self.pattern = text;
        self.status = if blank { SearchStatus::Idle } else { SearchStatus::Searching };
        request
    }

    pub fn found(&mut self, entry: Entry) {
        if self.status != SearchStatus::Searching {
            return;
        }
        let position = self.results.partition_point(|shown| shown.path <= entry.path);
        self.results.insert(position, entry);
        self.selected = match self.selected {
            None => Some(0),
            Some(row) if position <= row => Some(row + 1),
            keep => keep,
        };
    }

    pub fn finished(&mut self, truncated: bool) {
        if self.status == SearchStatus::Searching {
            self.status = SearchStatus::Done { truncated };
        }
    }

    pub fn failed(&mut self, message: String) {
        if self.status == SearchStatus::Searching {
            self.status = SearchStatus::Failed(message);
        }
    }

    pub fn select(&mut self, row: usize) {
        if row < self.results.len() {
            self.selected = Some(row);
        }
    }

    pub fn move_selection(&mut self, down: bool) {
        let last = self.results.len().checked_sub(1);
        self.selected = match (self.selected, last) {
            (_, None) => None,
            (None, Some(_)) => Some(0),
            (Some(current), Some(last)) if down => Some((current + 1).min(last)),
            (Some(current), Some(_)) => Some(current.saturating_sub(1)),
        };
    }

    pub fn selected(&self) -> Option<&Entry> {
        self.selected.and_then(|row| self.results.get(row))
    }

    pub fn selected_row(&self) -> Option<usize> {
        self.selected
    }

    pub fn shown_parent(&self, entry: &Entry) -> String {
        let parent = result_parent(entry);
        let root: Vec<_> = self.root.segments().collect();
        let below: Vec<_> = parent.segments().collect();
        if !below.starts_with(&root) {
            return parent.to_string();
        }
        let rest: Vec<_> = below[root.len()..].iter().map(|segment| segment.to_string_lossy()).collect();
        if rest.is_empty() { ".".to_string() } else { rest.join("/") }
    }

    pub fn status_text(&self) -> String {
        let count = self.results.len();
        match &self.status {
            SearchStatus::Idle => format!("Type a name to search under {}", self.root),
            SearchStatus::Searching if count == 0 => "Searching…".to_string(),
            SearchStatus::Searching => format!("Searching… {count} found"),
            SearchStatus::Done { truncated: true } if count > 0 => {
                format!("Showing the first {count} matches; narrow the search")
            }
            SearchStatus::Done { .. } if count == 0 => "Nothing found".to_string(),
            SearchStatus::Done { .. } => format!("{count} found"),
            SearchStatus::Failed(message) => format!("Search failed: {message}"),
        }
    }
}

#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::ops::Range;

mod filter;
mod save;
mod tabs;

pub use filter::LogFilter;
pub use save::{default_file_name, save_text, write_lines};

#[cfg(test)]
pub use tabs::MAX_LOG_TABS;
pub use tabs::{LogTabs, tabs_full_text};

use porthmos_core::{LogId, LogLevel, LogLine, LogSource, SessionId, UnitName};

const FOLLOW_TOLERANCE: f32 = 2.0;
pub const MAX_BUFFER_BYTES: usize = 32 * 1024 * 1024;

pub fn follow_after_scroll(offset: f32, viewport: f32, content: f32) -> bool {
    offset + viewport >= content - FOLLOW_TOLERANCE
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceKind {
    #[default]
    File,
    Journal,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LogStatus {
    #[default]
    Idle,
    Starting,
    Running,
    Ended(String),
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabName {
    Empty,
    File(String),
    Journal(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLine {
    pub seq: u64,
    pub line: LogLine,
    pub skipped: Option<u64>,
}

#[derive(Debug)]
pub struct LogsState {
    pub running: Option<(LogId, SessionId)>,
    pub kind: SourceKind,
    pub path: String,
    pub unit: String,
    pub lines: VecDeque<StoredLine>,
    pub next_seq: u64,
    pub held_back: u64,
    pub bytes: usize,
    pub byte_budget: usize,
    pub status: LogStatus,
    pub paused_at: Option<u64>,
    pub follow: bool,
    pub scroll: f32,
    pub snap_pending: bool,
    pub was_visible: bool,
    pub filter: LogFilter,
    pub(crate) matches: VecDeque<u64>,
}

impl Default for LogsState {
    fn default() -> Self {
        Self {
            running: None,
            kind: SourceKind::File,
            path: String::new(),
            unit: String::new(),
            lines: VecDeque::new(),
            next_seq: 0,
            held_back: 0,
            bytes: 0,
            byte_budget: MAX_BUFFER_BYTES,
            status: LogStatus::Idle,
            paused_at: None,
            follow: true,
            scroll: 0.0,
            snap_pending: false,
            was_visible: false,
            filter: LogFilter::default(),
            matches: VecDeque::new(),
        }
    }
}

impl LogsState {
    pub fn source(&self) -> Option<LogSource> {
        match self.kind {
            SourceKind::File => {
                let path = self.path.trim();
                (!path.is_empty()).then(|| LogSource::File(path.to_string()))
            }
            SourceKind::Journal => {
                let unit = self.unit.trim();
                if unit.is_empty() {
                    return Some(LogSource::Journal(None));
                }
                UnitName::new(unit).map(|name| LogSource::Journal(Some(name)))
            }
        }
    }

    pub fn name(&self) -> TabName {
        match self.kind {
            SourceKind::File => {
                let path = self.path.trim();
                if path.is_empty() {
                    return TabName::Empty;
                }
                TabName::File(path.rsplit('/').find(|part| !part.is_empty()).unwrap_or(path).to_string())
            }
            SourceKind::Journal => {
                let unit = self.unit.trim();
                TabName::Journal((!unit.is_empty()).then(|| unit.to_string()))
            }
        }
    }

    pub fn is_pristine(&self) -> bool {
        self.running.is_none()
            && self.lines.is_empty()
            && self.status == LogStatus::Idle
            && self.path.trim().is_empty()
            && self.unit.trim().is_empty()
    }

    pub fn append(&mut self, lines: Vec<LogLine>, skipped: u64, max_lines: usize) {
        if skipped > 0 {
            self.keep(marker(), Some(skipped), skipped, max_lines);
        }
        for line in lines {
            self.keep(line, None, 1, max_lines);
        }
        self.trim(max_lines);
        self.snap_pending = true;
    }

    pub fn trim(&mut self, max_lines: usize) {
        while self.lines.len() > max_lines || self.bytes > self.byte_budget && self.lines.len() > 1 {
            if let Some(oldest) = self.lines.pop_front() {
                self.bytes -= oldest.line.text.len();
            }
        }
        let first = self.lines.front().map_or(u64::MAX, |stored| stored.seq);
        while self.matches.front().is_some_and(|&seq| seq < first) {
            self.matches.pop_front();
        }
    }

    fn keep(&mut self, line: LogLine, skipped: Option<u64>, count: u64, max_lines: usize) {
        let full = self.lines.len() >= max_lines || self.bytes + line.text.len() > self.byte_budget;
        if self.paused() && full {
            self.held_back += count;
        } else {
            self.push(line, skipped);
        }
    }

    fn push(&mut self, line: LogLine, skipped: Option<u64>) {
        self.bytes += line.text.len();
        let stored = StoredLine { seq: self.next_seq, line, skipped };
        if self.filter.is_active() && self.filter.passes(&stored) {
            self.matches.push_back(stored.seq);
        }
        self.lines.push_back(stored);
        self.next_seq += 1;
    }

    pub fn clear_buffer(&mut self) {
        self.lines.clear();
        self.matches.clear();
        self.held_back = 0;
        self.bytes = 0;
        self.scroll = 0.0;
        if self.paused_at.is_some() {
            self.paused_at = Some(self.next_seq);
        }
    }

    pub fn start_fresh(&mut self) {
        self.clear_buffer();
        self.paused_at = None;
        self.follow = true;
        self.scroll = 0.0;
    }

    pub fn scrolled(&mut self, offset: f32, viewport: f32, content: f32) {
        self.scroll = offset;
        self.follow = follow_after_scroll(offset, viewport, content);
    }

    pub fn follow_toggled(&mut self, follow: bool) {
        self.follow = follow;
        self.snap_pending = follow;
    }

    pub fn take_snap(&mut self) -> bool {
        std::mem::take(&mut self.snap_pending) && self.follow
    }

    pub fn appeared(&mut self, visible: bool) -> bool {
        let before = std::mem::replace(&mut self.was_visible, visible);
        visible && !before
    }

    pub fn shown_len(&self) -> usize {
        match (self.paused_at, self.lines.front()) {
            (Some(at), Some(first)) => {
                usize::try_from(at.saturating_sub(first.seq)).unwrap_or(usize::MAX).min(self.lines.len())
            }
            (Some(_), None) => 0,
            (None, _) => self.lines.len(),
        }
    }

    pub fn filtering(&self) -> bool {
        self.filter.is_active()
    }

    pub fn set_filter_text(&mut self, text: String) {
        self.filter.text = text;
        self.filter_changed();
    }

    pub fn toggle_level(&mut self, level: LogLevel) {
        self.filter.toggle(level);
        self.filter_changed();
    }

    fn filter_changed(&mut self) {
        self.matches.clear();
        if self.filter.is_active() {
            self.matches =
                self.lines.iter().filter(|stored| self.filter.passes(stored)).map(|stored| stored.seq).collect();
        }
        self.follow = true;
        self.snap_pending = true;
    }

    pub fn view_len(&self) -> usize {
        if !self.filtering() {
            return self.shown_len();
        }
        let limit = self.paused_at.unwrap_or(u64::MAX);
        self.matches.partition_point(|&seq| seq < limit)
    }

    pub fn view_lines(&self, range: Range<usize>) -> Vec<&StoredLine> {
        if !self.filtering() {
            return self.lines.range(range.start.min(self.lines.len())..range.end.min(self.lines.len())).collect();
        }
        let Some(first) = self.lines.front().map(|stored| stored.seq) else {
            return Vec::new();
        };
        let end = range.end.min(self.view_len());
        let start = range.start.min(end);
        self.matches
            .range(start..end)
            .filter_map(|&seq| {
                seq.checked_sub(first)
                    .and_then(|index| usize::try_from(index).ok())
                    .and_then(|index| self.lines.get(index))
            })
            .collect()
    }

    pub fn paused(&self) -> bool {
        self.paused_at.is_some()
    }

    pub fn pause_toggled(&mut self, max_lines: usize) {
        if self.paused_at.take().is_some() {
            let held_back = std::mem::take(&mut self.held_back);
            if held_back > 0 {
                self.push(marker(), Some(held_back));
                self.trim(max_lines);
            }
        } else {
            self.paused_at = Some(self.next_seq);
        }
    }

    #[cfg(test)]
    pub fn shown(&self) -> impl Iterator<Item = &StoredLine> {
        let limit = self.paused_at.unwrap_or(u64::MAX);
        self.lines.iter().take_while(move |stored| stored.seq < limit)
    }

    pub fn unseen_since_pause(&self) -> usize {
        if !self.paused() {
            return 0;
        }
        let held_back = usize::try_from(self.held_back).unwrap_or(usize::MAX);
        (self.lines.len() - self.shown_len()).saturating_add(held_back)
    }
}

fn marker() -> LogLine {
    LogLine { text: String::new(), level: LogLevel::Unknown }
}

#[cfg(test)]
mod tests;

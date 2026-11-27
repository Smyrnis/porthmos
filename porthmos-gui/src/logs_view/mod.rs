use std::collections::VecDeque;

use porthmos_core::{LogId, LogLevel, LogLine, LogSource, SessionId, UnitName};

const FOLLOW_TOLERANCE: f32 = 2.0;
pub const MAX_BUFFER_BYTES: usize = 64 * 1024 * 1024;

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
pub struct StoredLine {
    pub seq: u64,
    pub line: LogLine,
    pub skipped: Option<u64>,
}

#[derive(Debug)]
pub struct LogsState {
    pub running: Option<(LogId, SessionId)>,
    pub next_id: LogId,
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
}

impl Default for LogsState {
    fn default() -> Self {
        Self {
            running: None,
            next_id: 1,
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
        self.lines.push_back(StoredLine { seq: self.next_seq, line, skipped });
        self.next_seq += 1;
    }

    pub fn clear_buffer(&mut self) {
        self.lines.clear();
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

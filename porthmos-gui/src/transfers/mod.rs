use std::ops::Range;

use porthmos_core::transfer::TransferSnapshot;
use porthmos_core::transfer::rows::{QueueRow, RowKind, RowState, percent_of};
use porthmos_core::{Command, Entry, Location, VfsPath};

use crate::geometry::visible_range;

const SEPARATOR: &str = " · ";
const SPEED_LIMIT_HINT: &str = "Use a number with K, M or G (for example 500K), or off";
const QUEUE_OVERSCAN: usize = 4;
const KIBI: u64 = 1024;
pub const QUEUE_ROW_HEIGHT: f32 = 28.0;
pub const QUEUE_HEIGHT: f32 = 220.0;
pub const QUEUE_HEADER_HEIGHT: f32 = 30.0;
pub const QUEUE_LIST_HEIGHT: f32 = QUEUE_HEIGHT - QUEUE_HEADER_HEIGHT - 1.0;

pub fn transfer_command(moving: bool, from: (Location, Vec<Entry>), to: (Location, VfsPath)) -> Command {
    let (from, entries) = from;
    let (to, dest_dir) = to;
    if moving { Command::Move { from, entries, to, dest_dir } } else { Command::Copy { from, entries, to, dest_dir } }
}

#[derive(Debug, Default)]
pub struct TransferState {
    snapshot: TransferSnapshot,
}

impl TransferState {
    pub fn replace(&mut self, snapshot: TransferSnapshot) {
        self.snapshot = snapshot;
    }

    pub fn rows(&self) -> &[QueueRow] {
        &self.snapshot.rows
    }

    pub fn snapshot(&self) -> &TransferSnapshot {
        &self.snapshot
    }

    pub fn speed_limit(&self) -> Option<u64> {
        self.snapshot.speed_limit
    }

    pub fn has_unfinished(&self) -> bool {
        self.snapshot.rows.iter().any(|row| !row.is_finished())
    }

    pub fn summary(&self) -> Option<String> {
        let working: Vec<&QueueRow> = self
            .snapshot
            .rows
            .iter()
            .filter(|row| !row.is_finished() && row.state != RowState::AwaitingAnswer)
            .collect();
        let mut parts = Vec::new();
        if !working.is_empty() {
            let noun = if working.len() == 1 { "transfer" } else { "transfers" };
            let mut text = format!("{} {noun}", working.len());
            let total: u64 = working.iter().map(|row| row.bytes_total).sum();
            if total > 0 {
                let done: u64 = working.iter().map(|row| row.bytes_done).sum();
                text.push_str(&format!("{SEPARATOR}{}%", percent_of(done, total)));
            }
            parts.push(text);
        }
        if self.snapshot.awaiting_answers > 0 {
            parts.push(format!("{} waiting for an answer", self.snapshot.awaiting_answers));
        }
        (!parts.is_empty()).then(|| parts.join(SEPARATOR))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowButton {
    Pause,
    Resume,
    Cancel,
    Retry,
    MoveUp,
    MoveDown,
}

impl RowButton {
    pub fn command(self, kind: RowKind) -> Command {
        match self {
            RowButton::Pause => Command::PauseRow { kind },
            RowButton::Resume => Command::ResumeRow { kind },
            RowButton::Cancel => Command::CancelRow { kind },
            RowButton::Retry => Command::RetryRow { kind },
            RowButton::MoveUp => Command::ReorderRow { kind, earlier: true },
            RowButton::MoveDown => Command::ReorderRow { kind, earlier: false },
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            RowButton::Pause => "Pause",
            RowButton::Resume => "Resume",
            RowButton::Cancel => "Cancel",
            RowButton::Retry => "Retry",
            RowButton::MoveUp => "▲",
            RowButton::MoveDown => "▼",
        }
    }
}

fn is_paused(row: &QueueRow) -> bool {
    row.paused || row.state == RowState::Paused
}

pub fn state_word(row: &QueueRow) -> &'static str {
    match row.state {
        RowState::Scanning => "Scanning",
        RowState::AwaitingAnswer => "Waiting for an answer",
        RowState::Running | RowState::Queued if row.paused => "Paused",
        RowState::Running => "Running",
        RowState::Queued => "Queued",
        RowState::Paused => "Paused",
        RowState::Done => "Done",
        RowState::PartlyFailed(_) => "Partly failed",
        RowState::Failed => "Failed",
        RowState::Cancelled => "Cancelled",
    }
}

pub fn failure_detail(row: &QueueRow) -> Option<String> {
    row.failure.as_ref().map(|failure| {
        let attempts = if failure.attempts == 1 { "attempt" } else { "attempts" };
        format!("{} {attempts}{SEPARATOR}{}", failure.attempts, failure.message)
    })
}

pub fn failure_text(row: &QueueRow) -> Option<String> {
    let detail = failure_detail(row)?;
    row.failure.as_ref().map(|failure| format!("{}{SEPARATOR}{detail}", failure.file))
}

pub fn row_buttons(row: &QueueRow) -> Vec<RowButton> {
    match row.state {
        RowState::Scanning | RowState::AwaitingAnswer => vec![RowButton::Cancel],
        RowState::Done | RowState::Cancelled => Vec::new(),
        RowState::Failed | RowState::PartlyFailed(_) => vec![RowButton::Retry],
        RowState::Running | RowState::Queued | RowState::Paused => {
            let toggle = if is_paused(row) { RowButton::Resume } else { RowButton::Pause };
            vec![toggle, RowButton::Cancel, RowButton::MoveUp, RowButton::MoveDown]
        }
    }
}

pub fn parse_speed_limit(text: &str) -> Result<Option<u64>, String> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("off") {
        return Ok(None);
    }
    let bad = || SPEED_LIMIT_HINT.to_string();
    let (digits, multiplier) = match text.char_indices().last() {
        Some((at, unit)) if unit.is_ascii_alphabetic() => {
            let multiplier = match unit.to_ascii_uppercase() {
                'K' => KIBI,
                'M' => KIBI * KIBI,
                'G' => KIBI * KIBI * KIBI,
                _ => return Err(bad()),
            };
            (&text[..at], multiplier)
        }
        _ => (text, 1),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(bad());
    }
    let bytes = digits.parse::<u64>().ok().and_then(|count| count.checked_mul(multiplier)).ok_or_else(bad)?;
    if bytes == 0 {
        return Err("The speed limit must be above zero; type off for none".to_string());
    }
    Ok(Some(bytes))
}

pub fn format_speed_limit(limit: Option<u64>) -> String {
    match limit {
        None => "off".to_string(),
        Some(bytes) if bytes % (KIBI * KIBI) == 0 => format!("{}M", bytes / (KIBI * KIBI)),
        Some(bytes) if bytes % KIBI == 0 => format!("{}K", bytes / KIBI),
        Some(bytes) => bytes.to_string(),
    }
}

pub fn queue_window(total: usize, offset: f32) -> Range<usize> {
    visible_range(offset, QUEUE_LIST_HEIGHT, QUEUE_ROW_HEIGHT, total, QUEUE_OVERSCAN)
}

#[derive(Debug, Default)]
pub struct QueuePane {
    open: bool,
    dismissed: bool,
    scroll: f32,
    speed_text: Option<String>,
    speed_error: Option<String>,
}

impl QueuePane {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self, unfinished: bool) {
        if self.open {
            self.open = false;
            self.dismissed = unfinished;
        } else {
            self.open = true;
            self.dismissed = false;
        }
    }

    pub fn snapshot_changed(&mut self, unfinished: bool) {
        if !unfinished {
            self.dismissed = false;
        } else if !self.dismissed {
            self.open = true;
        }
    }

    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    pub fn set_scroll(&mut self, offset: f32) {
        self.scroll = offset.max(0.0);
    }

    pub fn speed_text(&self) -> Option<&str> {
        self.speed_text.as_deref()
    }

    pub fn speed_error(&self) -> Option<&str> {
        self.speed_error.as_deref()
    }

    pub fn type_speed(&mut self, text: String) {
        self.speed_text = Some(text);
        self.speed_error = None;
    }

    pub fn cancel_speed(&mut self) {
        self.speed_text = None;
        self.speed_error = None;
    }

    pub fn submit_speed(&mut self) -> Option<Option<u64>> {
        let text = self.speed_text.as_deref()?;
        match parse_speed_limit(text) {
            Ok(limit) => {
                self.speed_text = None;
                self.speed_error = None;
                Some(limit)
            }
            Err(message) => {
                self.speed_error = Some(message);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;

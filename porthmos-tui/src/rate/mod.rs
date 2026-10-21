use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

use porthmos_core::transfer::rows::{QueueRow, RowKind, RowState};

use crate::widgets::file_list::size_label;

const WINDOW: Duration = Duration::from_secs(5);
const MAX_ETA_SECONDS: u64 = 99 * 3600 + 59 * 60 + 59;

#[derive(Default)]
pub struct RateTracker {
    windows: HashMap<RowKind, VecDeque<(Instant, u64)>>,
}

impl RateTracker {
    pub fn observe(&mut self, at: Instant, rows: &[QueueRow]) {
        let running: HashSet<RowKind> =
            rows.iter().filter(|row| row.state == RowState::Running).map(|row| row.kind).collect();
        self.windows.retain(|kind, _| running.contains(kind));

        for row in rows.iter().filter(|row| row.state == RowState::Running) {
            let window = self.windows.entry(row.kind).or_default();
            if window.back().is_some_and(|(_, bytes)| row.bytes_done < *bytes) {
                window.clear();
            }
            window.push_back((at, row.bytes_done));
            while window.len() > 2 && window.front().is_some_and(|(sampled, _)| at.duration_since(*sampled) > WINDOW) {
                window.pop_front();
            }
        }
    }

    pub fn speed(&self, kind: RowKind) -> Option<u64> {
        let window = self.windows.get(&kind)?;
        let (first_at, first_bytes) = *window.front()?;
        let (last_at, last_bytes) = *window.back()?;
        let elapsed = last_at.checked_duration_since(first_at)?.as_secs_f64();
        if elapsed <= 0.0 {
            return None;
        }
        Some((last_bytes.saturating_sub(first_bytes) as f64 / elapsed) as u64)
    }

    pub fn aggregate_speed(&self) -> Option<u64> {
        self.windows.keys().filter_map(|kind| self.speed(*kind)).reduce(u64::saturating_add)
    }

    pub fn eta_seconds(&self, row: &QueueRow) -> Option<u64> {
        if row.bytes_total == 0 {
            return None;
        }
        let speed = self.speed(row.kind).filter(|speed| *speed > 0)?;
        Some(row.bytes_total.saturating_sub(row.bytes_done) / speed)
    }
}

pub fn format_speed(bytes_per_second: u64) -> String {
    format!("{}/s", size_label(bytes_per_second))
}

pub fn format_eta(seconds: u64) -> String {
    let seconds = seconds.min(MAX_ETA_SECONDS);
    let (hours, minutes, rest) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{rest:02}") } else { format!("{minutes}:{rest:02}") }
}

#[cfg(test)]
mod tests;

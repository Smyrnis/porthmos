use std::time::Instant;

use porthmos_core::i18n;
use porthmos_core::{
    Disk, HealthSnapshot, Memory, MonitorState, PROCESS_ACTION_MAX_AGE, ProcessSort, SessionId, may_signal, size_text,
};

const UNKNOWN: &str = "\u{2026}";
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const FINE_PERCENT_BELOW: f32 = 10.0;

#[derive(Debug, Default)]
pub struct HealthState {
    kept: Option<Box<HealthSnapshot>>,
    pub chosen: Option<SessionId>,
    pub session: Option<SessionId>,
    pub started: Option<SessionId>,
    pub state: Option<MonitorState>,
    arrived: Option<Instant>,
    pub sort: ProcessSort,
    pub filter: String,
    pub selected: Option<u32>,
}

impl HealthState {
    pub fn begin(&mut self, session: SessionId) {
        self.clear();
        self.session = Some(session);
    }

    pub fn clear(&mut self) {
        self.session = None;
        self.state = None;
        self.arrived = None;
        self.kept = None;
        self.selected = None;
    }

    pub fn apply(&mut self, session: SessionId, state: MonitorState) {
        if self.session != Some(session) {
            return;
        }
        if let (Some(pid), MonitorState::Snapshot(snapshot)) = (self.selected, &state)
            && !snapshot.processes.iter().any(|process| process.pid == pid)
        {
            self.selected = None;
        }
        if let Some(MonitorState::Snapshot(snapshot)) = self.state.take() {
            self.kept = Some(snapshot);
        }
        if !matches!(state, MonitorState::Failing(_)) {
            self.kept = None;
        }
        if matches!(state, MonitorState::Snapshot(_)) {
            self.arrived = Some(Instant::now());
        }
        self.state = Some(state);
    }

    pub fn snapshot(&self) -> Option<&HealthSnapshot> {
        match &self.state {
            Some(MonitorState::Snapshot(snapshot)) => Some(snapshot),
            Some(MonitorState::Failing(_)) => self.kept.as_deref(),
            _ => None,
        }
    }

    pub fn list_is_old(&self, now: Instant) -> bool {
        self.arrived.is_some_and(|arrived| now.saturating_duration_since(arrived) > PROCESS_ACTION_MAX_AGE)
    }

    pub fn failing(&self) -> bool {
        matches!(self.state, Some(MonitorState::Failing(_)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionState {
    NoSelection,
    NeedsRoot,
    Allowed,
}

pub fn action_state(snapshot: &HealthSnapshot, selected: Option<u32>) -> ActionState {
    let Some(process) = selected.and_then(|pid| snapshot.processes.iter().find(|process| process.pid == pid)) else {
        return ActionState::NoSelection;
    };
    if may_signal(snapshot.uid, process.uid) { ActionState::Allowed } else { ActionState::NeedsRoot }
}

pub fn percent_text(value: Option<f32>) -> String {
    match value.filter(|value| value.is_finite() && *value >= 0.0) {
        None => UNKNOWN.to_string(),
        Some(value) if value < FINE_PERCENT_BELOW => format!("{value:.1}%"),
        Some(value) => format!("{value:.0}%"),
    }
}

pub fn uptime_text(seconds: u64) -> String {
    let (days, hours, minutes) = (seconds / DAY, seconds % DAY / HOUR, seconds % HOUR / MINUTE);
    let (days, hours, minutes) = (days.to_string(), hours.to_string(), minutes.to_string());
    if days != "0" {
        i18n::text_with("gui.health.uptime_days", &[("days", &days), ("hours", &hours)])
    } else if hours != "0" {
        i18n::text_with("gui.health.uptime_hours", &[("hours", &hours), ("minutes", &minutes)])
    } else {
        i18n::text_with("gui.health.uptime_minutes", &[("minutes", &minutes)])
    }
}

pub fn fraction(used: u64, total: u64) -> f32 {
    if total == 0 { 0.0 } else { (used as f64 / total as f64).clamp(0.0, 1.0) as f32 }
}

fn usage_text(used: u64, total: u64) -> String {
    let percent = (fraction(used, total) * 100.0).round() as u32;
    format!("{} / {} ({percent}%)", size_text(used), size_text(total))
}

pub fn disk_text(disk: &Disk) -> String {
    usage_text(disk.used, disk.total)
}

pub fn memory_used(memory: &Memory) -> u64 {
    memory.total.saturating_sub(memory.available)
}

pub fn memory_text(memory: &Memory) -> String {
    usage_text(memory_used(memory), memory.total)
}

pub fn swap_used(memory: &Memory) -> u64 {
    memory.swap_total.saturating_sub(memory.swap_free)
}

pub fn swap_text(memory: &Memory) -> Option<String> {
    (memory.swap_total > 0).then(|| usage_text(swap_used(memory), memory.swap_total))
}

#[cfg(test)]
pub(crate) mod tests;

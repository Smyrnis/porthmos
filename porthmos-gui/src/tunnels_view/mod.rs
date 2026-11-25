use std::collections::HashMap;
use std::time::Duration;

use porthmos_core::i18n::label as tr;
use porthmos_core::size_text;
use porthmos_core::{TunnelInfo, TunnelKey, TunnelKind, TunnelSpec, TunnelStats, TunnelStatus};

#[derive(Debug, Default)]
pub struct TunnelsState {
    pub rows: Vec<TunnelInfo>,
    rates: HashMap<TunnelKey, (u64, u64)>,
}

impl TunnelsState {
    pub fn apply_list(&mut self, rows: Vec<TunnelInfo>) {
        self.rows = rows;
        let rows = &self.rows;
        self.rates.retain(|key, _| rows.iter().any(|row| row.spec.key() == *key));
    }

    pub fn apply_status(&mut self, key: &TunnelKey, status: TunnelStatus) {
        let Some(row) = self.rows.iter_mut().find(|row| row.spec.key() == *key) else {
            return;
        };
        if !matches!(status, TunnelStatus::Running { .. }) {
            self.rates.remove(key);
        }
        row.status = status;
    }

    pub fn apply_stats(&mut self, batch: &[(TunnelKey, TunnelStats)], elapsed: Duration) {
        for (key, stats) in batch {
            let Some(row) = self.rows.iter_mut().find(|row| row.spec.key() == *key) else {
                continue;
            };
            let up = per_second(row.stats.bytes_up, stats.bytes_up, elapsed);
            let down = per_second(row.stats.bytes_down, stats.bytes_down, elapsed);
            self.rates.insert(key.clone(), (up, down));
            row.stats = *stats;
        }
    }

    pub fn rate(&self, key: &TunnelKey) -> (u64, u64) {
        self.rates.get(key).copied().unwrap_or_default()
    }

    pub fn running_count(&self) -> usize {
        self.rows.iter().filter(|row| matches!(row.status, TunnelStatus::Running { .. })).count()
    }

    pub fn tab_label(&self) -> String {
        match self.running_count() {
            0 => tr("Tunnels"),
            count => format!("{} · {count}", tr("Tunnels")),
        }
    }
}

fn per_second(before: u64, after: u64, elapsed: Duration) -> u64 {
    let milliseconds = elapsed.as_millis();
    if milliseconds == 0 {
        return 0;
    }
    (u128::from(after.saturating_sub(before)) * 1000 / milliseconds).try_into().unwrap_or(u64::MAX)
}

pub fn path_text(spec: &TunnelSpec) -> String {
    match &spec.kind {
        TunnelKind::Local { listen, target } => format!("{listen} → {target}"),
        TunnelKind::Dynamic { listen } => format!("{listen} (SOCKS)"),
    }
}

pub fn rate_text(bytes_per_second: u64) -> String {
    format!("{}/s", size_text(bytes_per_second))
}

pub fn total_text(bytes: u64) -> String {
    size_text(bytes)
}

#[cfg(test)]
mod tests;

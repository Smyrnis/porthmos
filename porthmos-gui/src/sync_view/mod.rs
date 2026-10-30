use std::sync::Arc;

use chrono::{Local, TimeZone};
use porthmos_core::{
    edit::printable,
    glob_match,
    sync::{SyncAction, SyncFacts, SyncItem, SyncPlan, SyncReason},
};

use crate::view::format::size;

const MISSING_TIME: &str = "\u{2014}";

#[derive(Default)]
pub struct SyncView {
    plan: Option<Arc<SyncPlan>>,
    ticked: Vec<bool>,
    actions: Vec<SyncAction>,
    visible: Vec<usize>,
    filter: Option<String>,
    cursor: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub ticked: usize,
    pub uploads: usize,
    pub upload_bytes: u64,
    pub downloads: usize,
    pub download_bytes: u64,
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRow {
    pub ticked: bool,
    pub action: SyncAction,
    pub reason: &'static str,
    pub path: String,
    pub size: u64,
    pub local_time: String,
    pub remote_time: String,
    pub flippable: bool,
}

pub fn reason_text(reason: SyncReason) -> &'static str {
    match reason {
        SyncReason::OnlyLocal => "new locally",
        SyncReason::OnlyRemote => "new on remote",
        SyncReason::LocalNewer => "local is newer",
        SyncReason::RemoteNewer => "remote is newer",
        SyncReason::TargetNewer => "target is newer",
        SyncReason::SameTimeDifferentSize => "same time, other size",
        SyncReason::SizeDiffers => "size differs",
        SyncReason::SizeDiffersTimeUnknown => "size differs, no time",
        SyncReason::KindMismatch => "file vs folder",
    }
}

pub fn summary_text(summary: &Summary) -> String {
    format!(
        "{} ticked \u{b7} \u{2191} {} ({}) \u{b7} \u{2193} {} ({}) \u{b7} {} skipped",
        summary.ticked,
        summary.uploads,
        size(summary.upload_bytes, false),
        summary.downloads,
        size(summary.download_bytes, false),
        summary.skipped,
    )
}

fn matches(item: &SyncItem, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    let path = item.path.to_string_lossy();
    let reason = reason_text(item.reason);
    if filter.contains(['*', '?']) {
        glob_match(filter, &path) || glob_match(filter, reason)
    } else {
        let needle = filter.to_lowercase();
        path.to_lowercase().contains(&needle) || reason.starts_with(&needle) || reason.contains(&format!(" {needle}"))
    }
}

fn moment(facts: Option<SyncFacts>) -> String {
    facts
        .and_then(|facts| facts.modified)
        .and_then(|seconds| Local.timestamp_opt(i64::try_from(seconds).ok()?, 0).single())
        .map_or_else(|| MISSING_TIME.to_string(), |time| time.format("%Y-%m-%d %H:%M").to_string())
}

impl SyncView {
    pub fn load(&mut self, plan: Arc<SyncPlan>) {
        self.ticked = plan.items.iter().map(|item| item.ticked).collect();
        self.actions = plan.items.iter().map(|item| item.action).collect();
        self.plan = Some(plan);
        self.filter = None;
        self.cursor = 0;
        self.refilter(None);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn has_plan(&self) -> bool {
        self.plan.is_some()
    }

    pub fn sync_id(&self) -> Option<u64> {
        self.plan.as_ref().map(|plan| plan.sync_id)
    }

    pub fn plan(&self) -> Option<&SyncPlan> {
        self.plan.as_deref()
    }

    pub fn total(&self) -> usize {
        self.ticked.len()
    }

    pub fn matched(&self) -> usize {
        self.visible.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    pub fn selected_item(&self) -> Option<usize> {
        self.visible.get(self.cursor).copied()
    }

    pub fn select(&mut self, row: usize) {
        if row < self.visible.len() {
            self.cursor = row;
        }
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let last = self.visible.len() as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, last) as usize;
    }

    pub fn toggle(&mut self) {
        let Some(index) = self.selected_item() else {
            return;
        };
        if self.ticked[index] {
            self.ticked[index] = false;
        } else if self.actions[index] != SyncAction::Skip {
            self.ticked[index] = true;
        } else {
            self.flip_at(index);
        }
    }

    pub fn flip(&mut self) {
        if let Some(index) = self.selected_item() {
            self.flip_at(index);
        }
    }

    pub fn can_flip(&self) -> bool {
        match (self.plan.as_ref(), self.selected_item()) {
            (Some(plan), Some(index)) => plan.items[index].flippable,
            _ => false,
        }
    }

    fn flip_at(&mut self, index: usize) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        let item = &plan.items[index];
        if !item.flippable {
            return;
        }
        let next = item.action_after(self.actions[index], plan.options.direction);
        self.actions[index] = next;
        self.ticked[index] = next != SyncAction::Skip;
    }

    pub fn tick_all(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        for index in self.visible.iter().copied() {
            let default_action = plan.items[index].action;
            if default_action != SyncAction::Skip {
                self.actions[index] = default_action;
                self.ticked[index] = true;
            }
        }
    }

    pub fn untick_all(&mut self) {
        for index in self.visible.iter().copied() {
            self.ticked[index] = false;
        }
    }

    pub fn visible_choices(&self) -> Vec<(u32, SyncAction)> {
        let Some(plan) = self.plan.as_ref() else {
            return Vec::new();
        };
        self.visible
            .iter()
            .copied()
            .filter(|index| self.ticked[*index] && self.actions[*index] != SyncAction::Skip)
            .map(|index| (plan.items[index].id, self.actions[index]))
            .collect()
    }

    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        let Some(plan) = self.plan.as_ref() else {
            return summary;
        };
        for (index, item) in plan.items.iter().enumerate() {
            let action = self.actions[index];
            if !self.ticked[index] || action == SyncAction::Skip {
                summary.skipped += 1;
                continue;
            }
            summary.ticked += 1;
            match action {
                SyncAction::Upload => {
                    summary.uploads += 1;
                    summary.upload_bytes += item.local.map_or(0, |facts| facts.size);
                }
                SyncAction::Download => {
                    summary.downloads += 1;
                    summary.download_bytes += item.remote.map_or(0, |facts| facts.size);
                }
                SyncAction::Skip => {}
            }
        }
        summary
    }

    pub fn top_text(&self) -> String {
        let summary = summary_text(&self.summary());
        if self.filter.is_some() {
            format!("{summary} \u{b7} {} will run", self.visible_choices().len())
        } else {
            summary
        }
    }

    pub fn set_filter(&mut self, text: String) {
        let keep = self.selected_item();
        self.filter = Some(text).filter(|text| !text.is_empty());
        self.refilter(keep);
    }

    pub fn row(&self, position: usize) -> Option<SyncRow> {
        let plan = self.plan.as_ref()?;
        let index = *self.visible.get(position)?;
        let item = &plan.items[index];
        let action = self.actions[index];
        let facts = match action {
            SyncAction::Download => item.remote.or(item.local),
            _ => item.local.or(item.remote),
        };
        Some(SyncRow {
            ticked: self.ticked[index],
            action,
            reason: reason_text(item.reason),
            path: printable(&item.path.to_string_lossy()),
            size: facts.map_or(0, |facts| facts.size),
            local_time: moment(item.local),
            remote_time: moment(item.remote),
            flippable: item.flippable,
        })
    }

    fn refilter(&mut self, keep: Option<usize>) {
        self.visible = match self.plan.as_ref() {
            Some(plan) => plan
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| self.filter.as_deref().is_none_or(|text| matches(item, text)))
                .map(|(index, _)| index)
                .collect(),
            None => Vec::new(),
        };
        let kept = keep.and_then(|index| self.visible.iter().position(|visible| *visible == index));
        self.cursor = kept.unwrap_or_else(|| self.cursor.min(self.visible.len().saturating_sub(1)));
    }
}

#[cfg(test)]
mod tests;

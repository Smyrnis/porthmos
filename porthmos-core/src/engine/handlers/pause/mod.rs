use super::super::Engine;
use crate::{
    tasks::Scope,
    transfer::{JobStatus, rows::RowKind},
};

impl Engine {
    pub(crate) fn pause_row(&mut self, kind: RowKind) {
        if matches!(kind, RowKind::Scan(_)) {
            return;
        }
        let targets: Vec<(u64, bool)> = self
            .transfers
            .jobs_of(kind)
            .iter()
            .filter(|job| matches!(job.status, JobStatus::Queued | JobStatus::InProgress))
            .map(|job| (job.id, job.status == JobStatus::InProgress))
            .collect();
        if targets.is_empty() {
            return;
        }
        for (id, running) in targets.iter().copied() {
            if let Some(mut job) = self.transfers.get_mut(id) {
                job.paused = true;
            }
            if running && !self.source_removals.contains(&id) {
                self.pause_stops.insert(id);
                self.tasks.cancel(Scope::Transfer(id));
            }
        }
        self.publish_transfers();
        for (id, running) in targets {
            if !running {
                self.refresh_transfer_destination(id);
            }
        }
    }

    pub(crate) fn resume_row(&mut self, kind: RowKind) {
        let paused: Vec<u64> = self.transfers.jobs_of(kind).iter().filter(|job| job.paused).map(|job| job.id).collect();
        if paused.is_empty() {
            return;
        }
        for id in paused {
            if let Some(mut job) = self.transfers.get_mut(id) {
                job.paused = false;
            }
        }
        self.publish_transfers();
        self.fill_transfer_slots();
    }

    pub(crate) fn requeue_stopped(&mut self, id: u64) -> bool {
        if !self.pause_stops.remove(&id) {
            return false;
        }
        if let Some(mut job) = self.transfers.get_mut(id) {
            job.status = JobStatus::Queued;
            job.attempts = job.attempts.saturating_sub(1);
            if job.remove_source {
                job.transferred_bytes = 0;
            } else if job.transferred_bytes > 0 {
                job.resume = true;
            }
        }
        self.refresh_transfer_destination(id);
        self.fill_transfer_slots();
        true
    }
}

#[cfg(test)]
mod tests;

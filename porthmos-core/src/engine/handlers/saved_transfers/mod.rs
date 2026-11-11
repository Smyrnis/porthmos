use std::collections::{BTreeSet, HashMap};

use porthmos_vfs::VfsPath;

use super::super::Engine;
use crate::{
    Severity,
    persist::{Loaded, write_atomic},
    transfer::{
        JobStatus,
        saved::{INTERRUPTED_REASON, SavedJob, load, render, unfinished},
    },
};

const SAVED_FILE_MODE: u32 = 0o600;

fn joined_with_or(names: &BTreeSet<String>) -> String {
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    match names.as_slice() {
        [] => String::new(),
        [only] => (*only).to_string(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

fn restored_notice(count: usize, connections: &BTreeSet<String>) -> String {
    let names = joined_with_or(connections);
    if count == 1 {
        format!("1 transfer from last time is in the transfers list. Connect to {names} and retry it to continue.")
    } else {
        format!(
            "{count} transfers from last time are in the transfers list. Connect to {names} and retry them to continue."
        )
    }
}

impl Engine {
    pub(crate) fn save_unfinished_transfers(&mut self) {
        if self.unfinished_protected {
            return;
        }
        let path = self.paths.unfinished_transfers_file();
        let jobs = unfinished(&self.transfers, |session_id| self.session_name(session_id));
        if jobs.is_empty() {
            if let Err(err) = std::fs::remove_file(&path)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                tracing::debug!("couldn't remove {}: {err}", path.display());
            }
            return;
        }
        let written = render(&jobs).and_then(|text| write_atomic(&path, text.as_bytes(), SAVED_FILE_MODE));
        if let Err(err) = written {
            self.report(Severity::Warning, format!("Couldn't save the unfinished transfers: {err:#}"));
        }
    }

    pub(crate) fn restore_unfinished_transfers(&mut self) {
        let jobs = match load(&self.paths) {
            Loaded::Missing => return,
            Loaded::SetAside { warning, protected } => {
                self.unfinished_protected = protected;
                self.report(Severity::Warning, warning);
                return;
            }
            Loaded::Ready(jobs) => jobs,
        };
        if jobs.is_empty() {
            return;
        }
        let mut sessions: HashMap<String, u64> = HashMap::new();
        let mut batches: HashMap<u64, u64> = HashMap::new();
        for saved in &jobs {
            let session_id = match sessions.get(&saved.connection) {
                Some(id) => *id,
                None => {
                    let id = self.next_session_id;
                    self.next_session_id += 1;
                    self.history.remember_session(id, saved.connection.clone());
                    sessions.insert(saved.connection.clone(), id);
                    id
                }
            };
            let batch_id = saved.batch.map(|old| {
                *batches.entry(old).or_insert_with(|| {
                    self.transfers.start_batch(saved.label.clone().unwrap_or_else(|| saved.display_name.clone()))
                })
            });
            self.enqueue_restored(session_id, batch_id, saved);
        }
        for row in self.transfers.rows() {
            self.history.skip_recording(row.kind);
        }
        self.process_row_changes();
        let connections: BTreeSet<String> = jobs.iter().map(|saved| saved.connection.clone()).collect();
        self.info(restored_notice(jobs.len(), &connections));
        self.transfers_dirty = true;
        self.publish_transfers();
    }

    fn enqueue_restored(&mut self, session_id: u64, batch_id: Option<u64>, saved: &SavedJob) {
        let id = self.transfers.enqueue(
            session_id,
            saved.direction,
            VfsPath::new(&saved.local_path),
            saved.remote_path.clone(),
            saved.display_name.clone(),
            saved.total_bytes,
            batch_id,
        );
        let Some(mut job) = self.transfers.get_mut(id) else {
            return;
        };
        job.status = JobStatus::Failed(saved.failure.clone().unwrap_or_else(|| INTERRUPTED_REASON.to_string()));
        job.attempts = 1;
        job.resume = true;
        job.transferred_bytes = saved.transferred_bytes;
        job.remove_source = saved.remove_source;
        job.modified = saved.modified;
    }
}

#[cfg(test)]
mod tests;

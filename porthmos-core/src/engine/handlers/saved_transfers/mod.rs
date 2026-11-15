use crate::i18n;
use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use tokio::time::Instant;

use porthmos_vfs::VfsPath;

use super::{super::Engine, MoveBatch};
use crate::{
    Severity,
    persist::{Loaded, write_atomic},
    transfer::{
        JobStatus,
        saved::{SavedJob, SavedMove, load, load_moves, render_with_moves, unfinished},
    },
};

const SAVED_FILE_MODE: u32 = 0o600;
pub(super) const UNFINISHED_PROGRESS_SAVE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(crate) struct UnfinishedSaves {
    ready: bool,
    shape: Vec<SavedJob>,
    moves: Vec<SavedMove>,
    transferred: u64,
    saved_at: Option<Instant>,
}

fn joined_with_or(names: &BTreeSet<String>) -> String {
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    match names.as_slice() {
        [] => String::new(),
        [only] => (*only).to_string(),
        [rest @ .., last] => i18n::text_with("common.or", &[("rest", &rest.join(", ")), ("last", last)]),
    }
}

fn restored_notice(count: usize, connections: &BTreeSet<String>) -> String {
    let names = joined_with_or(connections);
    i18n::plural_with("transfers.restored", count, &[("names", &names)])
}

impl Engine {
    fn saved_moves(&self, jobs: &[SavedJob]) -> Vec<SavedMove> {
        let mut moves: Vec<SavedMove> = self
            .work
            .moves
            .iter()
            .filter(|(batch, moved)| !moved.roots.is_empty() && jobs.iter().any(|job| job.batch == Some(**batch)))
            .map(|(batch, moved)| SavedMove {
                batch: *batch,
                connection: self.session_name(moved.session_id),
                direction: moved.direction,
                roots: moved.roots.iter().map(|root| root.to_string_lossy().into_owned()).collect(),
            })
            .collect();
        moves.sort_by_key(|moved| moved.batch);
        moves
    }

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
        let moves = self.saved_moves(&jobs);
        let written =
            render_with_moves(&jobs, &moves).and_then(|text| write_atomic(&path, text.as_bytes(), SAVED_FILE_MODE));
        if let Err(err) = written {
            self.report(Severity::Warning, i18n::text_with("transfers.save_failed", &[("cause", &format!("{err:#}"))]));
        }
    }

    pub(crate) fn save_unfinished_when_changed(&mut self) {
        if !self.unfinished_saves.ready || self.unfinished_protected {
            return;
        }
        let jobs = unfinished(&self.transfers, |session_id| self.session_name(session_id));
        let transferred: u64 = jobs.iter().map(|job| job.transferred_bytes).sum();
        let shape: Vec<SavedJob> = jobs.iter().map(|job| SavedJob { transferred_bytes: 0, ..job.clone() }).collect();
        let moves = self.saved_moves(&jobs);
        let saves = &self.unfinished_saves;
        let changed = shape != saves.shape || moves != saves.moves;
        let progressed = transferred != saves.transferred
            && saves.saved_at.is_none_or(|saved_at| saved_at.elapsed() >= UNFINISHED_PROGRESS_SAVE);
        if !changed && !progressed {
            return;
        }
        self.unfinished_saves.shape = shape;
        self.unfinished_saves.moves = moves;
        self.unfinished_saves.transferred = transferred;
        self.unfinished_saves.saved_at = Some(Instant::now());
        self.save_unfinished_transfers();
    }

    pub(crate) fn restore_unfinished_transfers(&mut self) {
        self.restore_saved_jobs();
        self.unfinished_saves.ready = true;
    }

    fn restore_saved_jobs(&mut self) {
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
                    let id = self.book.allocate_id();
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
        for moved in load_moves(&self.paths) {
            let (Some(batch_id), Some(session_id)) = (batches.get(&moved.batch), sessions.get(&moved.connection))
            else {
                continue;
            };
            let roots = moved.roots.iter().map(|root| VfsPath::new(root)).collect();
            self.work.moves.insert(*batch_id, MoveBatch { session_id: *session_id, direction: moved.direction, roots });
        }
        for row in self.transfers.rows() {
            self.history.skip_recording(row.kind);
        }
        self.process_row_changes();
        let connections: BTreeSet<String> = jobs.iter().map(|saved| saved.connection.clone()).collect();
        self.info(restored_notice(jobs.len(), &connections));
        self.work.transfers_dirty = true;
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
        job.status =
            JobStatus::Failed(saved.failure.clone().unwrap_or_else(crate::transfer::saved::interrupted_reason));
        job.attempts = 1;
        job.resume = true;
        job.transferred_bytes = saved.transferred_bytes;
        job.remove_source = saved.remove_source;
        job.modified = saved.modified;
    }
}

#[cfg(test)]
mod tests;

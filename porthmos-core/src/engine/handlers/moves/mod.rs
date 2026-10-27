use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::atomic::{AtomicBool, Ordering},
};

use porthmos_vfs::{Entry, ErrorKind, FileKind, FileSystem, ProtocolError, VfsPath};

use super::super::{Engine, Event, Internal, Location, TransferEvent};
use crate::{
    Severity,
    tasks::Scope,
    transfer::{
        Direction, JobStatus,
        plan::{PlannedFile, discover},
        rows::RowKind,
    },
};

pub(crate) struct MoveBatch {
    pub(crate) session_id: u64,
    pub(crate) direction: Direction,
    pub(crate) roots: Vec<VfsPath>,
}

pub(crate) async fn remove_copied_source(
    source: &dyn FileSystem, source_path: &VfsPath, destination: &dyn FileSystem, destination_path: &VfsPath,
) -> Result<(), String> {
    let original = match source.stat(source_path).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.to_string()),
    };
    let copy =
        destination.stat(destination_path).await.map_err(|err| format!("the copy couldn't be checked ({err})"))?;
    if copy.is_dir() || copy.size != original.size {
        return Err("the copy at the destination doesn't match the original".to_string());
    }
    source.remove_file(source_path).await.map_err(|err| err.to_string())
}

pub(crate) async fn remove_emptied_directories(fs: &dyn FileSystem, roots: &[VfsPath]) -> usize {
    let cancel = AtomicBool::new(false);
    let mut removed = 0;
    for root in roots {
        let Ok(Some(tree)) = discover(fs, root, true, &cancel).await else {
            continue;
        };
        let directories =
            tree.directories.iter().rev().map(|relative| root.join(relative)).chain(std::iter::once(root.clone()));
        for directory in directories {
            if fs.remove_dir(&directory).await.is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

pub(crate) fn is_loopback(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']').to_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host.parse::<IpAddr>().is_ok_and(|address| address.is_loopback())
}

pub(crate) async fn without_symlinks(
    fs: &dyn FileSystem, entries: Vec<Entry>,
) -> Result<(Vec<Entry>, usize), ProtocolError> {
    let mut links_by_parent: HashMap<VfsPath, HashSet<String>> = HashMap::new();
    for entry in &entries {
        let Some(parent) = entry.path.parent() else {
            continue;
        };
        if links_by_parent.contains_key(&parent) {
            continue;
        }
        let links = fs
            .read_dir(&parent)
            .await?
            .into_iter()
            .filter(|item| item.metadata.kind == FileKind::Symlink)
            .map(|item| item.name)
            .collect();
        links_by_parent.insert(parent, links);
    }
    let total = entries.len();
    let kept: Vec<Entry> = entries
        .into_iter()
        .filter(|entry| {
            let is_link = entry
                .path
                .parent()
                .and_then(|parent| links_by_parent.get(&parent))
                .is_some_and(|links| links.contains(&entry.name));
            !is_link
        })
        .collect();
    let skipped = total - kept.len();
    Ok((kept, skipped))
}

fn case_collisions(files: &[PlannedFile]) -> HashSet<VfsPath> {
    let mut groups: HashMap<(VfsPath, String), Vec<&VfsPath>> = HashMap::new();
    for file in files {
        let (Some(parent), Some(name)) = (file.destination.parent(), file.destination.file_name()) else {
            continue;
        };
        groups.entry((parent, name.to_string_lossy().to_lowercase())).or_default().push(&file.destination);
    }
    groups.into_values().filter(|group| group.len() > 1).flatten().cloned().collect()
}

impl Engine {
    pub(crate) fn move_entries(&mut self, from: Location, entries: Vec<Entry>, to: Location, dest_dir: VfsPath) {
        self.copy_or_move(from, entries, to, dest_dir, true);
    }

    pub(crate) fn vet_move(&self, session_id: u64, files: Vec<PlannedFile>) -> (Vec<PlannedFile>, HashSet<VfsPath>) {
        let on_this_computer = self.sessions.get(&session_id).is_some_and(|session| is_loopback(&session.entry.host));
        let (onto_themselves, files): (Vec<PlannedFile>, Vec<PlannedFile>) = if on_this_computer {
            files.into_iter().partition(|file| file.source == file.destination)
        } else {
            (Vec::new(), files)
        };
        if !onto_themselves.is_empty() {
            let plural = if onto_themselves.len() == 1 { "" } else { "s" };
            self.report(
                Severity::Warning,
                format!("Skipped {} file{plural} that would be moved onto itself", onto_themselves.len()),
            );
        }
        let colliding = case_collisions(&files);
        if !colliding.is_empty() {
            self.report(
                Severity::Warning,
                format!(
                    "Kept the originals of {} files whose names differ only by case; the destination may treat them as one file",
                    colliding.len()
                ),
            );
        }
        (files, colliding)
    }

    pub(crate) fn forget_batch(&mut self, batch_id: u64) {
        self.transfers.forget_batch_if_empty(batch_id);
        if self.transfers.batch_label(batch_id).is_none() {
            self.moves.remove(&batch_id);
        }
    }

    pub(crate) fn verb_of(&self, batch_id: u64) -> &'static str {
        if self.moves.contains_key(&batch_id) { "Move" } else { "Copy" }
    }

    pub(crate) fn removes_source(&self, id: u64) -> bool {
        self.transfers.get(id).is_some_and(|job| job.remove_source)
    }

    pub(crate) fn remove_moved_source(&mut self, id: u64) {
        let Some(job) = self.transfers.get(id) else {
            return;
        };
        let (session_id, direction) = (job.session_id, job.direction);
        let (source_path, destination_path) = job.source_and_destination();
        let Some((source, destination)) = self.filesystems_for(session_id, direction) else {
            return self.source_removal_failed(id, "the session disconnected".to_string());
        };
        self.source_removals.insert(id);
        let internal = self.internal.clone();
        self.tasks.spawn("remove-source", Scope::Transfer(id), move |cancel| async move {
            let event = if cancel.load(Ordering::Relaxed) {
                TransferEvent::SourceRemovalCancelled { id }
            } else {
                let outcome =
                    remove_copied_source(source.as_ref(), &source_path, destination.as_ref(), &destination_path).await;
                match outcome {
                    Ok(()) => TransferEvent::SourceRemoved { id },
                    Err(message) => TransferEvent::SourceRemovalFailed { id, message },
                }
            };
            let _ = internal.send(Internal::Transfer(event));
        });
    }

    pub(crate) fn source_removed(&mut self, id: u64) {
        self.tasks.forget(Scope::Transfer(id));
        self.source_removals.remove(&id);
        self.pause_stops.remove(&id);
        if let Some(mut job) = self.transfers.get_mut(id) {
            job.status = JobStatus::Completed;
            job.paused = false;
        }
        self.moved_job_settled(id);
    }

    pub(crate) fn source_removal_cancelled(&mut self, id: u64) {
        self.tasks.forget(Scope::Transfer(id));
        self.source_removals.remove(&id);
        if self.requeue_stopped(id) {
            return;
        }
        if let Some(mut job) = self.transfers.get_mut(id) {
            job.status = JobStatus::Cancelled;
        }
        self.moved_job_settled(id);
    }

    pub(crate) fn source_removal_failed(&mut self, id: u64, message: String) {
        self.tasks.forget(Scope::Transfer(id));
        self.source_removals.remove(&id);
        self.pause_stops.remove(&id);
        let Some(mut job) = self.transfers.get_mut(id) else {
            return;
        };
        job.paused = false;
        let name = job.display_name.clone();
        job.status = JobStatus::Failed(format!("copied, but the original couldn't be removed: {message}"));
        drop(job);
        self.announce(Severity::Error, format!("Copied {name}, but couldn't remove the original: {message}"));
        self.moved_job_settled(id);
    }

    fn moved_job_settled(&mut self, id: u64) {
        self.refresh_transfer_destination(id);
        if let Some(job) = self.transfers.get(id) {
            let (session_id, direction) = (job.session_id, job.direction);
            if !self.transfers.has_pending(session_id, direction) {
                self.refresh_source(session_id, direction);
            }
        }
        self.fill_transfer_slots();
        self.finish_move_batch_of(id);
    }

    pub(crate) fn refresh_source(&self, session_id: u64, direction: Direction) {
        match direction {
            Direction::Upload => self.emit(Event::LocationChanged { location: Location::Local }),
            Direction::Download => {
                if self.sessions.contains_key(&session_id) {
                    self.emit(Event::LocationChanged { location: Location::Session(session_id) });
                }
            }
        }
    }

    pub(crate) fn finish_move_batch_of(&mut self, id: u64) {
        if let Some(batch_id) = self.transfers.get(id).and_then(|job| job.batch_id) {
            self.finish_move_batch(batch_id);
        }
    }

    pub(crate) fn finish_move_batch(&mut self, batch_id: u64) {
        let Some(batch) = self.moves.get(&batch_id) else {
            return;
        };
        if batch.roots.is_empty() {
            return;
        }
        let unfinished = self
            .transfers
            .jobs_of(RowKind::Batch(batch_id))
            .iter()
            .any(|job| matches!(job.status, JobStatus::Queued | JobStatus::InProgress));
        if unfinished {
            return;
        }
        let (session_id, direction, roots) = (batch.session_id, batch.direction, batch.roots.clone());
        let source = match direction {
            Direction::Upload => self.local_fs.clone(),
            Direction::Download => match self.sessions.get(&session_id) {
                Some(session) => session.fs.clone(),
                None => return,
            },
        };
        let internal = self.internal.clone();
        self.tasks.spawn("remove-moved-folders", Scope::Background, move |_| async move {
            let removed = remove_emptied_directories(source.as_ref(), &roots).await;
            let _ = internal.send(Internal::Transfer(TransferEvent::MovedFoldersRemoved {
                session_id,
                direction,
                removed,
            }));
        });
    }

    pub(crate) fn finish_all_move_batches(&mut self) {
        let batches: Vec<u64> = self.moves.keys().copied().collect();
        for batch_id in batches {
            self.finish_move_batch(batch_id);
        }
    }

    pub(crate) fn finish_move_batches_of(&mut self, session_id: u64) {
        let batches: Vec<u64> =
            self.moves.iter().filter(|(_, batch)| batch.session_id == session_id).map(|(id, _)| *id).collect();
        for batch_id in batches {
            self.finish_move_batch(batch_id);
        }
    }

    pub(crate) fn forget_cleared_moves(&mut self) {
        let transfers = &self.transfers;
        self.moves.retain(|batch_id, _| transfers.batch_label(*batch_id).is_some());
    }
}

#[cfg(test)]
mod tests;

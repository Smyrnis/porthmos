use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use porthmos_lfs::to_os_path;
use porthmos_vfs::{Entry, FileSystem, PART_SUFFIX, ProtocolError, VfsPath};

use super::{
    super::{ConflictReview, Engine, Event, Internal, Location, PlanningScan, TransferEvent},
    MoveBatch, without_symlinks,
};
use crate::{
    Severity,
    tasks::Scope,
    transfer::{
        self, Direction, JobStatus, TransferJob, TransferOutcome,
        conflicts::{ConflictInfo, Resolution},
        job::Destination,
        plan::{DirectoryPlan, PlanOutcome},
        rows::{QueueRow, RowKind},
    },
    user_message,
};

impl Engine {
    pub(super) fn filesystems_for(
        &self, session_id: u64, direction: Direction,
    ) -> Option<(Arc<dyn FileSystem>, Arc<dyn FileSystem>)> {
        let remote = self.sessions.get(&session_id)?.fs.clone();
        Some(match direction {
            Direction::Upload => (self.local_fs.clone(), remote),
            Direction::Download => (remote, self.local_fs.clone()),
        })
    }

    pub(crate) fn copy(&mut self, from: Location, entries: Vec<Entry>, to: Location, dest_dir: VfsPath) {
        self.copy_or_move(from, entries, to, dest_dir, false);
    }

    pub(crate) fn copy_or_move(
        &mut self, from: Location, entries: Vec<Entry>, to: Location, dest_dir: VfsPath, moving: bool,
    ) {
        let (session_id, direction) = match (from, to) {
            (Location::Local, Location::Session(session_id)) => (session_id, Direction::Upload),
            (Location::Session(session_id), Location::Local) => (session_id, Direction::Download),
            _ => {
                let gerund = if moving { "Moving" } else { "Copying" };
                self.report(Severity::Warning, format!("{gerund} between two remote sessions isn't supported yet"));
                return;
            }
        };
        if entries.is_empty() {
            return;
        }

        self.start_copy_plan(session_id, direction, entries, dest_dir, moving);
        self.fill_transfer_slots();
    }

    fn start_copy_plan(
        &mut self, session_id: u64, direction: Direction, entries: Vec<Entry>, dest_dir: VfsPath, moving: bool,
    ) {
        let verb = if moving { "Move" } else { "Copy" };
        let Some((source, destination)) = self.filesystems_for(session_id, direction) else {
            self.report(Severity::Error, format!("{verb} failed: session disconnected"));
            return;
        };

        let mut display_name = match entries.as_slice() {
            [entry] => entry.name.clone(),
            _ => format!("{} items", entries.len()),
        };
        if moving {
            display_name.push_str(" (move)");
        }
        let batch_id = self.transfers.start_batch(display_name.clone());
        if moving {
            let roots = entries.iter().filter(|entry| entry.is_dir).map(|entry| entry.path.clone()).collect();
            self.moves.insert(batch_id, MoveBatch { session_id, direction, roots });
        }
        self.planning.push(PlanningScan { batch_id, session_id, direction, display_name });

        let internal = self.internal.clone();
        self.tasks.spawn("plan-copy", Scope::Planning(batch_id), move |cancel| async move {
            let planned = async {
                let (entries, skipped_selected) =
                    if moving { without_symlinks(source.as_ref(), entries).await? } else { (entries, 0) };
                let outcome =
                    transfer::plan::plan_copy(source.as_ref(), destination.as_ref(), entries, &dest_dir, &cancel)
                        .await?;
                Ok::<_, ProtocolError>((outcome, skipped_selected))
            }
            .await;
            let event = match planned {
                Ok((PlanOutcome::Ready(mut plan), skipped_selected)) => {
                    plan.skipped_symlinks += skipped_selected;
                    TransferEvent::PlanReady { batch_id, session_id, direction, plan }
                }
                Ok((PlanOutcome::Cancelled, _)) => TransferEvent::PlanCancelled { batch_id, session_id, direction },
                Err(err) => {
                    tracing::debug!("{err:?}");
                    TransferEvent::PlanFailed { batch_id, message: user_message(format!("{verb} failed"), &err) }
                }
            };
            let _ = internal.send(Internal::Transfer(event));
        });
    }

    pub(crate) fn fill_transfer_slots(&mut self) {
        loop {
            let sessions = &self.sessions;
            let startable = self.transfers.startable_limited(self.max_parallel, |session_id| {
                sessions.get(&session_id).and_then(|session| session.fs.transfer_limit())
            });
            if startable.is_empty() {
                return;
            }
            for id in startable {
                self.start_transfer(id);
            }
        }
    }

    fn start_transfer(&mut self, id: u64) {
        let Some(job) = self.transfers.get(id) else {
            return;
        };
        let session_id = job.session_id;
        let display_name = job.display_name.clone();

        let Some((source, destination)) = self.filesystems_for(session_id, job.direction) else {
            if let Some(mut job) = self.transfers.get_mut(id) {
                job.status = JobStatus::Failed("session disconnected".to_string());
            }
            self.report(Severity::Error, format!("Transfer failed: {display_name} \u{2014} session disconnected"));
            self.refresh_transfer_destination(id);
            return;
        };

        let Some(mut job) = self.transfers.get_mut(id) else {
            return;
        };
        job.status = JobStatus::InProgress;
        job.attempts += 1;
        let (source_path, destination_path) = job.source_and_destination();
        let resume = job.resume && !job.remove_source;
        let keep_modified = job.modified;
        drop(job);

        let internal = self.internal.clone();
        let limit = self.speed_limit.clone();
        self.tasks.spawn("transfer", Scope::Transfer(id), move |cancel| async move {
            let progress = internal.clone();
            let result = transfer::execute_preserving(
                source.as_ref(),
                &source_path,
                destination.as_ref(),
                &destination_path,
                &cancel,
                resume,
                keep_modified,
                &limit,
                move |transferred| {
                    let _ = progress.send(Internal::Transfer(TransferEvent::Progress { id, transferred }));
                },
            )
            .await;

            let event = match result {
                Ok(executed) => {
                    if !executed.times_kept {
                        let _ = internal.send(Internal::Transfer(TransferEvent::TimesNotKept { id }));
                    }
                    TransferEvent::Finished { id, outcome: executed.outcome }
                }
                Err(err) => {
                    tracing::debug!("{err:?}");
                    TransferEvent::Failed {
                        id,
                        message: user_message(format!("Transfer failed: {display_name}"), &err),
                    }
                }
            };
            let _ = internal.send(Internal::Transfer(event));
        });
    }

    pub(crate) fn handle_transfer_event(&mut self, event: TransferEvent) {
        match event {
            TransferEvent::Progress { id, transferred } => {
                if let Some(mut job) = self.transfers.get_mut(id) {
                    job.transferred_bytes = transferred;
                }
            }
            TransferEvent::Finished { id, outcome } => {
                self.tasks.forget(Scope::Transfer(id));
                if outcome == TransferOutcome::Cancelled && self.requeue_stopped(id) {
                    return;
                }
                if outcome == TransferOutcome::Completed && self.removes_source(id) {
                    self.pause_stops.remove(&id);
                    return self.remove_moved_source(id);
                }
                self.pause_stops.remove(&id);
                if let Some(mut job) = self.transfers.get_mut(id) {
                    job.status = match outcome {
                        TransferOutcome::Completed => JobStatus::Completed,
                        TransferOutcome::Cancelled => JobStatus::Cancelled,
                    };
                    job.paused = false;
                }
                self.refresh_transfer_destination(id);
                self.fill_transfer_slots();
                self.finish_move_batch_of(id);
            }
            TransferEvent::SourceRemoved { id } => self.source_removed(id),
            TransferEvent::SourceRemovalFailed { id, message } => self.source_removal_failed(id, message),
            TransferEvent::SourceRemovalCancelled { id } => self.source_removal_cancelled(id),
            TransferEvent::MovedFoldersRemoved { session_id, direction, removed } => {
                if removed > 0 {
                    self.refresh_source(session_id, direction);
                }
            }
            TransferEvent::Failed { id, message } => {
                let cancelled = self.tasks.take_cancelled(Scope::Transfer(id));
                if cancelled && self.requeue_stopped(id) {
                    tracing::debug!("{message}");
                    return;
                }
                self.pause_stops.remove(&id);
                let known = self.transfers.get(id).is_some();
                if cancelled && let Some(mut job) = self.transfers.get_mut(id) {
                    job.status = JobStatus::Cancelled;
                    job.paused = false;
                }
                let retried = !cancelled && self.transfers.fail_or_retry(id, message.clone());
                if !retried {
                    if !cancelled && known {
                        self.announce(Severity::Error, message);
                    } else if !cancelled {
                        self.report(Severity::Error, message);
                    }
                    self.refresh_transfer_destination(id);
                }
                self.fill_transfer_slots();
                if !retried {
                    self.finish_move_batch_of(id);
                }
            }
            TransferEvent::PlanReady { batch_id, session_id, direction, plan } => {
                self.clear_planning(batch_id);

                if !self.sessions.contains_key(&session_id) {
                    self.report(Severity::Error, format!("{} failed: session disconnected", self.verb_of(batch_id)));
                    self.forget_batch(batch_id);
                    return;
                }

                self.review_or_apply_plan(batch_id, session_id, direction, plan);
            }
            TransferEvent::PlanFailed { batch_id, message } => {
                let scan = self
                    .planning
                    .iter()
                    .find(|scan| scan.batch_id == batch_id)
                    .map(|scan| (scan.session_id, scan.direction, scan.display_name.clone()));
                self.clear_planning(batch_id);
                self.forget_batch(batch_id);
                match scan {
                    Some((session_id, direction, label)) => {
                        self.record_failed_scan(session_id, direction, label, &message);
                        self.announce(Severity::Error, message);
                    }
                    None => self.report(Severity::Error, message),
                }
            }
            TransferEvent::SyncPlanReady { batch_id, plan } => self.sync_plan_ready(batch_id, plan),
            TransferEvent::SyncCancelled { batch_id } => self.sync_cancelled(batch_id),
            TransferEvent::SyncFailed { batch_id, message } => self.sync_failed(batch_id, message),
            TransferEvent::SyncFoldersReady { batch_id, session_id, direction, plan } => {
                self.sync_folders_ready(batch_id, session_id, direction, plan)
            }
            TransferEvent::TimesNotKept { id } => {
                let Some((batch_id, session_id, direction)) = self
                    .transfers
                    .get(id)
                    .and_then(|job| job.batch_id.map(|batch| (batch, job.session_id, job.direction)))
                else {
                    return;
                };
                if self.transfers.flag_unkept_times(batch_id) {
                    let connection = match direction {
                        Direction::Upload => self.session_name(session_id),
                        Direction::Download => "this computer".to_string(),
                    };
                    self.report(
                        Severity::Warning,
                        format!(
                            "Couldn't keep the modification times on {connection}; the next sync will list these files again"
                        ),
                    );
                }
            }
            TransferEvent::PartialsRemoved { session_id } => self.refresh_destination(session_id, Direction::Upload),
            TransferEvent::PlanCancelled { batch_id, session_id, direction } => {
                self.clear_planning(batch_id);
                let verb = self.verb_of(batch_id);
                self.forget_batch(batch_id);
                if self.sessions.contains_key(&session_id) {
                    self.info(format!("{verb} cancelled"));
                }
                self.refresh_destination(session_id, direction);
            }
        }
    }

    pub(crate) fn review_or_apply_plan(
        &mut self, batch_id: u64, session_id: u64, direction: Direction, plan: DirectoryPlan,
    ) {
        let conflicts = transfer::conflicts::conflict_indices(&plan);
        if conflicts.is_empty() {
            self.apply_plan_ready(batch_id, session_id, direction, plan, &[], false);
            return;
        }
        let automatic: Option<Vec<Resolution>> =
            conflicts.iter().map(|index| self.on_conflict.resolution_for(&plan.files[*index])).collect();
        if let Some(answers) = automatic {
            self.apply_plan_ready(batch_id, session_id, direction, plan, &answers, false);
            return;
        }
        let files: Vec<ConflictInfo> = conflicts.iter().map(|index| ConflictInfo::from(&plan.files[*index])).collect();
        self.reviews.push_back(ConflictReview { batch_id, session_id, direction, plan });
        self.emit(Event::ConflictsFound { batch_id, files });
    }

    pub(crate) fn resolve_conflicts(&mut self, batch_id: u64, answers: Option<Vec<Resolution>>) {
        let Some(index) = self.reviews.iter().position(|review| review.batch_id == batch_id) else {
            return;
        };
        let Some(review) = self.reviews.remove(index) else {
            return;
        };
        match answers {
            Some(answers) => self.apply_plan_ready(
                review.batch_id,
                review.session_id,
                review.direction,
                review.plan,
                &answers,
                false,
            ),
            None => {
                let verb = self.verb_of(review.batch_id);
                self.forget_batch(review.batch_id);
                self.info(format!("{verb} cancelled"));
                self.refresh_destination(review.session_id, review.direction);
            }
        }
    }

    pub(crate) fn drop_conflict_reviews(&mut self, should_drop: impl Fn(&ConflictReview) -> bool) -> usize {
        let dropped: Vec<u64> =
            self.reviews.iter().filter(|review| should_drop(review)).map(|review| review.batch_id).collect();
        self.reviews.retain(|review| !should_drop(review));
        for batch_id in &dropped {
            self.forget_batch(*batch_id);
        }
        if !dropped.is_empty() {
            self.emit(Event::ConflictsWithdrawn { batch_ids: dropped.clone() });
        }
        dropped.len()
    }

    fn destination_can_set_modified(&self, session_id: u64, direction: Direction) -> bool {
        match direction {
            Direction::Upload => self.sessions.get(&session_id).is_some_and(|session| session.fs.can_set_modified()),
            Direction::Download => self.local_fs.can_set_modified(),
        }
    }

    pub(crate) fn apply_plan_ready(
        &mut self, batch_id: u64, session_id: u64, direction: Direction, mut plan: DirectoryPlan,
        answers: &[Resolution], preserve_times: bool,
    ) {
        let skipped_symlinks = plan.skipped_symlinks;
        let renamed = std::mem::take(&mut plan.renamed);
        let keeps_times = preserve_times || self.destination_can_set_modified(session_id, direction);
        let moving = self.moves.contains_key(&batch_id);
        let resolved = transfer::conflicts::resolve(plan, answers);
        let (files, kept_originals) = if moving {
            self.vet_move(session_id, resolved.files)
        } else {
            (resolved.files, std::collections::HashSet::new())
        };
        if files.is_empty() {
            self.refresh_destination(session_id, direction);
            if moving {
                self.finish_move_batch(batch_id);
            }
        }

        for file in files {
            let resume = file.resume;
            let removes_source = moving && !kept_originals.contains(&file.destination);
            let keep_modified = if keeps_times { file.source_modified } else { None };
            let (local_path, remote_path) = match direction {
                Direction::Upload => (file.source, file.destination),
                Direction::Download => (file.destination, file.source),
            };
            let id = self.transfers.enqueue(
                session_id,
                direction,
                local_path,
                remote_path.to_string_lossy().into_owned(),
                file.display_name,
                file.size,
                Some(batch_id),
            );
            if resume && let Some(mut job) = self.transfers.get_mut(id) {
                job.resume = true;
            }
            if removes_source && let Some(mut job) = self.transfers.get_mut(id) {
                job.remove_source = true;
            }
            if keep_modified.is_some()
                && let Some(mut job) = self.transfers.get_mut(id)
            {
                job.modified = keep_modified;
            }
        }
        if skipped_symlinks > 0 {
            let plural = if skipped_symlinks == 1 { "" } else { "s" };
            self.report(Severity::Warning, format!("Skipped {skipped_symlinks} symlink{plural}"));
        }
        if !renamed.is_empty() {
            self.report(Severity::Warning, transfer::names::renamed_notice(&renamed));
        }
        if resolved.skipped > 0 {
            let plural = if resolved.skipped == 1 { "" } else { "s" };
            self.info(format!("Skipped {} existing file{plural}", resolved.skipped));
        }
        if resolved.skipped_partials > 0 {
            let plural = if resolved.skipped_partials == 1 { "" } else { "s" };
            self.info(format!("Skipped {} partly copied file{plural}", resolved.skipped_partials));
        }
        if resolved.blocked_by_folder > 0 {
            let reason = if resolved.blocked_by_folder == 1 {
                "file because a folder with the same name exists"
            } else {
                "files because folders with the same names exist"
            };
            self.report(Severity::Warning, format!("Skipped {} {reason}", resolved.blocked_by_folder));
        }
        self.forget_batch(batch_id);
        self.fill_transfer_slots();
    }

    pub(crate) fn clear_planning(&mut self, batch_id: u64) {
        self.planning.retain(|scan| scan.batch_id != batch_id);
        self.tasks.forget(Scope::Planning(batch_id));
    }

    pub(super) fn refresh_transfer_destination(&mut self, id: u64) {
        let Some(job) = self.transfers.get(id) else {
            return;
        };
        let session_id = job.session_id;
        let direction = job.direction;

        if self.transfers.has_pending(session_id, direction) {
            return;
        }

        self.refresh_destination(session_id, direction);
    }

    pub(crate) fn refresh_destination(&self, session_id: u64, direction: Direction) {
        match direction {
            Direction::Upload => {
                if self.sessions.contains_key(&session_id) {
                    self.emit(Event::LocationChanged { location: Location::Session(session_id) });
                }
            }
            Direction::Download => self.emit(Event::LocationChanged { location: Location::Local }),
        }
    }

    pub(crate) fn cancel_all_copies(&mut self) {
        self.drop_conflict_reviews(|_| true);
        self.drop_sync_plans(|_| true);
        self.cancel_all_work();
        let mut cancelled_destinations: Vec<(u64, Direction)> = Vec::new();
        for job in self.transfers.queued_jobs() {
            let destination = (job.session_id, job.direction);
            if !cancelled_destinations.contains(&destination) {
                cancelled_destinations.push(destination);
            }
        }
        self.transfers.cancel_all_queued();
        self.refresh_destinations_without_pending(&cancelled_destinations);
        self.finish_all_move_batches();
    }

    pub(crate) fn cancel_all_work(&mut self) {
        let scans: Vec<u64> = self.planning.iter().map(|scan| scan.batch_id).collect();
        for batch_id in scans {
            self.tasks.cancel(Scope::Planning(batch_id));
        }
        let running: Vec<u64> = self.transfers.active_jobs().map(|job| job.id).collect();
        self.pause_stops.clear();
        for id in running {
            self.tasks.cancel(Scope::Transfer(id));
        }
        self.cancel_edit_downloads();
    }

    fn refresh_destinations_without_pending(&mut self, destinations: &[(u64, Direction)]) {
        for &(session_id, direction) in destinations {
            if !self.transfers.has_pending(session_id, direction) {
                self.refresh_destination(session_id, direction);
            }
        }
    }

    pub(crate) fn cancel_session_transfers(&mut self, session_id: u64) -> usize {
        let scans: Vec<u64> =
            self.planning.iter().filter(|scan| scan.session_id == session_id).map(|scan| scan.batch_id).collect();
        for batch_id in &scans {
            self.tasks.cancel(Scope::Planning(*batch_id));
        }
        let active_ids = self.transfers.active_ids_for_session(session_id);
        for id in &active_ids {
            self.pause_stops.remove(id);
            self.tasks.cancel(Scope::Transfer(*id));
        }
        scans.len() + active_ids.len()
    }

    pub(crate) fn cancel_row(&mut self, kind: RowKind) {
        if let RowKind::Scan(batch_id) = kind {
            if self.planning.iter().any(|scan| scan.batch_id == batch_id) {
                self.tasks.cancel(Scope::Planning(batch_id));
            }
            self.drop_conflict_reviews(|review| review.batch_id == batch_id);
            return;
        }
        let ids: Vec<u64> = self.transfers.jobs_of(kind).iter().map(|job| job.id).collect();
        let mut cancelled_destinations: Vec<(u64, Direction)> = Vec::new();
        for id in &ids {
            let Some(mut job) = self.transfers.get_mut(*id) else {
                continue;
            };
            match job.status {
                JobStatus::Queued => {
                    job.status = JobStatus::Cancelled;
                    job.paused = false;
                    let destination = (job.session_id, job.direction);
                    if !cancelled_destinations.contains(&destination) {
                        cancelled_destinations.push(destination);
                    }
                }
                JobStatus::InProgress => {
                    self.pause_stops.remove(id);
                    self.tasks.cancel(Scope::Transfer(*id));
                }
                _ => {}
            }
        }
        self.refresh_destinations_without_pending(&cancelled_destinations);
        if let RowKind::Batch(batch_id) = kind {
            self.finish_move_batch(batch_id);
        }
    }

    pub(crate) fn retry_row(&mut self, kind: RowKind) {
        let retryable: Vec<(u64, u64, Option<u64>)> = self
            .transfers
            .jobs_of(kind)
            .iter()
            .filter(|job| matches!(job.status, JobStatus::Failed(_) | JobStatus::Cancelled))
            .map(|job| (job.id, job.session_id, job.batch_id))
            .collect();
        let mut ready: Vec<u64> = Vec::new();
        let mut reconnect: Vec<String> = Vec::new();
        for (id, session_id, batch_id) in retryable {
            match self.usable_session_for(session_id) {
                Some(target) => {
                    if target != session_id {
                        self.transfers.rebind_session(id, target);
                        if let Some(moving) = batch_id.and_then(|batch_id| self.moves.get_mut(&batch_id)) {
                            moving.session_id = target;
                        }
                    }
                    ready.push(id);
                }
                None => {
                    let name = self.session_name(session_id);
                    if !reconnect.contains(&name) {
                        reconnect.push(name);
                    }
                }
            }
        }
        for name in reconnect {
            self.report(Severity::Warning, format!("Can't retry: reconnect to {name} first"));
        }
        if self.transfers.retry_jobs(&ready) > 0 {
            self.fill_transfer_slots();
        }
    }

    fn usable_session_for(&self, session_id: u64) -> Option<u64> {
        let usable = |id: &u64| self.sessions.contains_key(id) && !self.lost_sessions.contains(id);
        if usable(&session_id) {
            return Some(session_id);
        }
        let name = self.session_name(session_id);
        self.sessions.iter().rev().find(|(id, live)| live.name == name && usable(id)).map(|(id, _)| *id)
    }

    pub(crate) fn clear_finished_rows(&mut self) {
        let finished_ids: Vec<u64> = self
            .transfers
            .rows()
            .into_iter()
            .filter(QueueRow::is_finished)
            .flat_map(|row| self.transfers.jobs_of(row.kind).into_iter().map(|job| job.id).collect::<Vec<_>>())
            .collect();

        let interrupted: Vec<TransferJob> = finished_ids
            .iter()
            .filter_map(|id| self.transfers.get(*id))
            .filter(|job| matches!(job.status, JobStatus::Cancelled | JobStatus::Failed(_)))
            .cloned()
            .collect();
        let protected = self.destinations_still_in_use();
        self.transfers.remove_jobs(&finished_ids);
        self.forget_cleared_moves();
        let unused: Vec<TransferJob> =
            interrupted.into_iter().filter(|job| !protected.contains(&job.part_destination())).collect();
        self.remove_partials(unused);
    }

    fn destinations_still_in_use(&self) -> HashSet<Destination> {
        let mut protected: HashSet<Destination> = self.transfers.jobs().map(TransferJob::destination).collect();
        protected.extend(
            self.transfers
                .jobs()
                .filter(|job| matches!(job.status, JobStatus::Queued | JobStatus::InProgress))
                .map(TransferJob::part_destination),
        );
        protected
    }

    fn remove_partials(&mut self, interrupted: Vec<TransferJob>) {
        let mut removed_local = false;
        let mut remote_parts: HashMap<u64, Vec<String>> = HashMap::new();
        for job in interrupted {
            match job.direction {
                Direction::Download => {
                    let part = to_os_path(&job.local_path.with_suffix(PART_SUFFIX));
                    removed_local |= std::fs::remove_file(part).is_ok();
                }
                Direction::Upload => {
                    remote_parts.entry(job.session_id).or_default().push(format!("{}{PART_SUFFIX}", job.remote_path))
                }
            }
        }
        if removed_local {
            self.emit(Event::LocationChanged { location: Location::Local });
        }
        for (session_id, parts) in remote_parts {
            let Some(session) = self.sessions.get(&session_id) else {
                continue;
            };
            let fs = session.fs.clone();
            let internal = self.internal.clone();
            self.tasks.spawn("remove-partials", Scope::Background, move |_| async move {
                for part in parts {
                    let _ = fs.remove_file(&VfsPath::new(&part)).await;
                }
                let _ = internal.send(Internal::Transfer(TransferEvent::PartialsRemoved { session_id }));
            });
        }
    }
}

#[cfg(test)]
mod tests;

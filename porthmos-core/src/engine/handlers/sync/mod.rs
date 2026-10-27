use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use porthmos_vfs::{FileSystem, ProtocolError, VfsPath};

use super::super::{Engine, Event, Internal, PlanningScan, TransferEvent};
use crate::{
    Severity,
    sync::{self, SyncAction, SyncDirection, SyncItem, SyncOptions, SyncPlan, scan::scan_both},
    tasks::Scope,
    transfer::{
        Direction,
        conflicts::Resolution,
        plan::{DirectoryPlan, ExistingFile, PlannedFile, ensure_directory},
        rows::RowKind,
    },
    user_message,
};

const NAMED_UNREADABLE_FOLDERS: usize = 3;

fn unreadable_folders_notice(folders: &[VfsPath]) -> String {
    let plural = if folders.len() == 1 { "" } else { "s" };
    let named: Vec<String> = folders.iter().take(NAMED_UNREADABLE_FOLDERS).map(|folder| folder.to_string()).collect();
    let more = folders.len().saturating_sub(NAMED_UNREADABLE_FOLDERS);
    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
    format!("Skipped {} unreadable folder{plural}: {}{tail}", folders.len(), named.join(", "))
}

#[derive(Default)]
pub(crate) struct SyncState {
    plans: HashMap<u64, Arc<SyncPlan>>,
    scanning: HashSet<u64>,
}

impl SyncState {
    pub(crate) fn is_scanning(&self, batch_id: u64) -> bool {
        self.scanning.contains(&batch_id)
    }
}

fn planned_file(plan: &SyncPlan, item: &SyncItem, action: SyncAction) -> Option<PlannedFile> {
    let local = plan.local_root.join(&item.path);
    let remote = plan.remote_root.join(&item.path);
    let (source, destination, from, onto) = match action {
        SyncAction::Upload => (local, remote, item.local?, item.remote),
        SyncAction::Download => (remote, local, item.remote?, item.local),
        SyncAction::Skip => return None,
    };
    Some(PlannedFile {
        source,
        destination,
        display_name: item.path.to_string_lossy().into_owned(),
        size: from.size,
        existing: onto.map(|facts| ExistingFile { size: facts.size, modified: facts.modified, is_dir: false }),
        source_modified: from.modified,
        partial: None,
        resume: false,
    })
}

fn missing_folders(files: &[PlannedFile], root: &VfsPath) -> BTreeSet<VfsPath> {
    let mut folders = BTreeSet::new();
    for file in files {
        for ancestor in file.destination.ancestors().skip(1) {
            if ancestor == *root || !ancestor.starts_with(root) {
                break;
            }
            folders.insert(ancestor);
        }
    }
    folders
}

async fn create_folders(
    fs: &dyn FileSystem, folders: &BTreeSet<VfsPath>, cancel: &AtomicBool,
) -> Result<bool, ProtocolError> {
    for folder in folders {
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        ensure_directory(fs, folder).await?;
    }
    Ok(true)
}

impl Engine {
    pub(crate) fn start_sync(
        &mut self, session_id: u64, local_dir: VfsPath, remote_dir: VfsPath, options: SyncOptions,
    ) {
        if let Err(message) = options.validate() {
            self.report(Severity::Warning, message);
            return;
        }
        let Some(live) = self.sessions.get(&session_id) else {
            self.report(Severity::Warning, "Sync failed: session disconnected");
            return;
        };
        let (remote, name) = (live.fs.clone(), live.name.clone());
        let local = self.local_fs.clone();
        if options.direction == SyncDirection::Both && !(local.can_set_modified() && remote.can_set_modified()) {
            self.report(Severity::Warning, format!("Both can't be used with {name}: it can't keep modification times"));
            return;
        }
        let tolerance = local.time_resolution().max(remote.time_resolution());
        let target_keeps_times = match options.direction {
            SyncDirection::LocalToRemote => remote.can_set_modified(),
            SyncDirection::RemoteToLocal => local.can_set_modified(),
            SyncDirection::Both => true,
        };
        let display_name = format!("sync {}", local_dir);
        let batch_id = self.transfers.start_batch(display_name.clone());
        self.planning.push(PlanningScan { batch_id, session_id, direction: Direction::Upload, display_name });
        self.sync.scanning.insert(batch_id);

        let internal = self.internal.clone();
        self.tasks.spawn("sync-scan", Scope::Planning(batch_id), move |cancel| async move {
            let scanned =
                scan_both(local.as_ref(), &local_dir, remote.as_ref(), &remote_dir, options.subfolders, &cancel).await;
            let event = match scanned {
                Ok(Some((local_scan, remote_scan))) => TransferEvent::SyncPlanReady {
                    batch_id,
                    plan: SyncPlan {
                        sync_id: batch_id,
                        session: session_id,
                        local_root: local_dir,
                        remote_root: remote_dir,
                        options,
                        items: sync::compare(
                            &local_scan.tree,
                            &remote_scan.tree,
                            options,
                            tolerance,
                            target_keeps_times,
                        ),
                        skipped_symlinks: local_scan.skipped_symlinks + remote_scan.skipped_symlinks,
                        unreadable_folders: local_scan.unreadable,
                    },
                },
                Ok(None) => TransferEvent::SyncCancelled { batch_id },
                Err(err) => {
                    tracing::debug!("{err:?}");
                    TransferEvent::SyncFailed { batch_id, message: user_message("Sync failed", &err) }
                }
            };
            let _ = internal.send(Internal::Transfer(event));
        });
    }

    pub(crate) fn sync_plan_ready(&mut self, batch_id: u64, plan: SyncPlan) {
        if self.tasks.take_cancelled(Scope::Planning(batch_id)) {
            self.sync_cancelled(batch_id);
            return;
        }
        self.sync.scanning.remove(&batch_id);
        self.clear_planning(batch_id);
        self.transfers.forget_batch_if_empty(batch_id);
        if !self.sessions.contains_key(&plan.session) {
            self.report(Severity::Error, "Sync failed: session disconnected");
            return;
        }
        if plan.skipped_symlinks > 0 {
            let plural = if plan.skipped_symlinks == 1 { "" } else { "s" };
            self.report(Severity::Warning, format!("Skipped {} symlink{plural}", plan.skipped_symlinks));
        }
        if !plan.unreadable_folders.is_empty() {
            self.report(Severity::Warning, unreadable_folders_notice(&plan.unreadable_folders));
        }
        if plan.items.is_empty() {
            self.info("Folders are in sync");
            return;
        }
        let plan = Arc::new(plan);
        self.sync.plans.insert(batch_id, plan.clone());
        self.emit(Event::SyncPlanReady(plan));
    }

    pub(crate) fn run_sync(&mut self, sync_id: u64, choices: Vec<(u32, SyncAction)>) {
        let Some(plan) = self.sync.plans.remove(&sync_id) else {
            self.report(Severity::Warning, "That sync plan is no longer available");
            return;
        };
        if !self.sessions.contains_key(&plan.session) {
            self.report(Severity::Error, "Sync failed: session disconnected");
            return;
        }
        let mut uploads = Vec::new();
        let mut downloads = Vec::new();
        let mut seen = HashSet::new();
        let mut refused = 0;
        for (id, action) in choices {
            let Some(item) = plan.items.get(id as usize).filter(|item| item.id == id) else {
                refused += 1;
                continue;
            };
            if !seen.insert(id) {
                refused += 1;
                continue;
            }
            if action == SyncAction::Skip {
                continue;
            }
            let unchanged = action == item.action;
            if !item.allows(action, plan.options.direction) || !(unchanged || item.flippable) {
                refused += 1;
                continue;
            }
            match planned_file(&plan, item, action) {
                Some(file) if action == SyncAction::Upload => uploads.push(file),
                Some(file) => downloads.push(file),
                None => refused += 1,
            }
        }
        if refused > 0 {
            let (plural, verb) = if refused == 1 { ("", "doesn't") } else { ("s", "don't") };
            self.report(Severity::Warning, format!("Ignored {refused} sync choice{plural} that {verb} apply"));
        }
        if uploads.is_empty() && downloads.is_empty() {
            self.info("Nothing to sync");
            return;
        }
        for (direction, files) in [(Direction::Upload, uploads), (Direction::Download, downloads)] {
            if !files.is_empty() {
                self.start_sync_batch(&plan, direction, files);
            }
        }
    }

    fn start_sync_batch(&mut self, plan: &SyncPlan, direction: Direction, files: Vec<PlannedFile>) {
        let Some(live) = self.sessions.get(&plan.session) else {
            return;
        };
        let (target_fs, target_root, arrow) = match direction {
            Direction::Upload => (live.fs.clone(), plan.remote_root.clone(), '\u{2191}'),
            Direction::Download => (self.local_fs.clone(), plan.local_root.clone(), '\u{2193}'),
        };
        let display_name = format!("sync {arrow} {}", target_root);
        let batch_id = self.transfers.start_batch(display_name.clone());
        let session_id = plan.session;
        self.planning.push(PlanningScan { batch_id, session_id, direction, display_name });
        self.sync.scanning.insert(batch_id);
        let folders = missing_folders(&files, &target_root);
        let internal = self.internal.clone();
        self.tasks.spawn("sync-folders", Scope::Planning(batch_id), move |cancel| async move {
            let event = match create_folders(target_fs.as_ref(), &folders, &cancel).await {
                Ok(true) => TransferEvent::SyncFoldersReady {
                    batch_id,
                    session_id,
                    direction,
                    plan: DirectoryPlan { files, skipped_symlinks: 0, taken_names: HashMap::new() },
                },
                Ok(false) => TransferEvent::SyncCancelled { batch_id },
                Err(err) => {
                    tracing::debug!("{err:?}");
                    TransferEvent::SyncFailed { batch_id, message: user_message("Sync failed", &err) }
                }
            };
            let _ = internal.send(Internal::Transfer(event));
        });
    }

    pub(crate) fn sync_folders_ready(
        &mut self, batch_id: u64, session_id: u64, direction: Direction, plan: DirectoryPlan,
    ) {
        if !self.sync.is_scanning(batch_id) {
            return;
        }
        if self.tasks.take_cancelled(Scope::Planning(batch_id)) {
            self.sync_cancelled(batch_id);
            return;
        }
        self.sync.scanning.remove(&batch_id);
        self.clear_planning(batch_id);
        if !self.sessions.contains_key(&session_id) {
            self.transfers.forget_batch_if_empty(batch_id);
            self.report(Severity::Error, "Sync failed: session disconnected");
            return;
        }
        let questions = crate::transfer::conflicts::conflict_indices(&plan).len();
        let answers = vec![Resolution::Overwrite; questions];
        self.apply_plan_ready(batch_id, session_id, direction, plan, &answers, true);
    }

    pub(crate) fn sync_cancelled(&mut self, batch_id: u64) {
        self.sync.scanning.remove(&batch_id);
        let session = self.planning.iter().find(|scan| scan.batch_id == batch_id).map(|scan| scan.session_id);
        self.clear_planning(batch_id);
        self.transfers.forget_batch_if_empty(batch_id);
        if session.is_some_and(|session| self.sessions.contains_key(&session)) {
            self.info("Sync cancelled");
        }
    }

    pub(crate) fn sync_failed(&mut self, batch_id: u64, message: String) {
        self.sync.scanning.remove(&batch_id);
        self.clear_planning(batch_id);
        self.transfers.forget_batch_if_empty(batch_id);
        self.report(Severity::Error, message);
    }

    pub(crate) fn cancel_sync(&mut self, sync_id: u64) {
        if self.sync.scanning.contains(&sync_id) {
            self.tasks.cancel(Scope::Planning(sync_id));
            return;
        }
        self.sync.plans.remove(&sync_id);
    }

    pub(crate) fn is_sync_scan(&self, kind: RowKind) -> bool {
        matches!(kind, RowKind::Scan(batch_id) if self.sync.is_scanning(batch_id))
    }

    pub(crate) fn drop_sync_plans(&mut self, should_drop: impl Fn(&SyncPlan) -> bool) {
        let dropped: Vec<u64> =
            self.sync.plans.values().filter(|plan| should_drop(plan)).map(|plan| plan.sync_id).collect();
        for sync_id in &dropped {
            self.sync.plans.remove(sync_id);
        }
        if !dropped.is_empty() {
            self.emit(Event::SyncWithdrawn { sync_ids: dropped });
        }
    }
}

#[cfg(test)]
mod tests;

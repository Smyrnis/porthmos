use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use porthmos_vfs::{ErrorKind, FileKind, FileSystem, ProtocolError, VfsPath};

use super::super::{Engine, Event, Internal, PlanningScan, TransferEvent};
use crate::{
    Location, Severity,
    delete_prompt::THIS_COMPUTER,
    sync::{self, SyncAction, SyncDirection, SyncItem, SyncOptions, SyncPlan, SyncReason, scan::scan_both},
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
const DELETIONS_SKIPPED: &str = "Deletions skipped because the copies did not all finish";

struct DeletionReport {
    files: usize,
    folders: usize,
    changed: usize,
    failure: Option<(VfsPath, ProtocolError)>,
}

fn plural(count: usize, word: &str) -> String {
    if count == 1 { format!("{count} {word}") } else { format!("{count} {word}s") }
}

fn deleted_summary(report: &DeletionReport, place: &str) -> Option<String> {
    let mut parts = Vec::new();
    if report.files > 0 {
        parts.push(plural(report.files, "file"));
    }
    if report.folders > 0 {
        parts.push(plural(report.folders, "folder"));
    }
    (!parts.is_empty()).then(|| format!("Deleted {} from {place}", parts.join(" and ")))
}

async fn is_target_only(source_fs: &dyn FileSystem, source_root: &VfsPath, relative: &VfsPath) -> bool {
    match source_fs.stat(&source_root.join(relative)).await {
        Ok(_) => false,
        Err(err) => err.kind() == ErrorKind::NotFound,
    }
}

async fn remove_if_empty(fs: &dyn FileSystem, folder: &VfsPath) -> Result<bool, ProtocolError> {
    match fs.list(folder).await {
        Ok(entries) if entries.is_empty() => {}
        _ => return Ok(false),
    }
    match fs.remove_dir(folder).await {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == ErrorKind::Unsupported => fs.delete(folder).await.map(|()| true),
        Err(err) => Err(err),
    }
}

async fn delete_mirrored(pending: &PendingDeletion) -> DeletionReport {
    let mut report = DeletionReport { files: 0, folders: 0, changed: 0, failure: None };
    for relative in &pending.files {
        let path = pending.target_root.join(relative);
        let still_a_target_only_file = match pending.target_fs.stat(&path).await {
            Ok(metadata) => metadata.kind == FileKind::File,
            Err(_) => false,
        } && is_target_only(pending.source_fs.as_ref(), &pending.source_root, relative)
            .await;
        if !still_a_target_only_file {
            report.changed += 1;
            continue;
        }
        if let Err(err) = pending.target_fs.delete(&path).await {
            report.failure = Some((path, err));
            return report;
        }
        report.files += 1;
    }
    let mut candidates: BTreeSet<VfsPath> = pending.folders.iter().cloned().collect();
    for relative in pending.files.iter().chain(&pending.folders) {
        for ancestor in relative.ancestors().skip(1).filter(|ancestor| !ancestor.is_empty()) {
            if !candidates.contains(&ancestor)
                && is_target_only(pending.source_fs.as_ref(), &pending.source_root, &ancestor).await
            {
                candidates.insert(ancestor);
            }
        }
    }
    let mut ordered: Vec<VfsPath> = candidates.into_iter().collect();
    ordered.sort_by_key(|folder| std::cmp::Reverse(folder.segments().count()));
    for relative in ordered {
        let path = pending.target_root.join(&relative);
        match remove_if_empty(pending.target_fs.as_ref(), &path).await {
            Ok(true) => report.folders += 1,
            Ok(false) => {}
            Err(err) => {
                report.failure = Some((path, err));
                return report;
            }
        }
    }
    report
}

fn unreadable_folders_notice(folders: &[VfsPath]) -> String {
    let plural = if folders.len() == 1 { "" } else { "s" };
    let named: Vec<String> = folders.iter().take(NAMED_UNREADABLE_FOLDERS).map(|folder| folder.to_string()).collect();
    let more = folders.len().saturating_sub(NAMED_UNREADABLE_FOLDERS);
    let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
    format!("Skipped {} unreadable folder{plural}: {}{tail}", folders.len(), named.join(", "))
}

pub(crate) struct PendingDeletion {
    location: Location,
    place: String,
    target_fs: Arc<dyn FileSystem>,
    source_fs: Arc<dyn FileSystem>,
    target_root: VfsPath,
    source_root: VfsPath,
    files: Vec<VfsPath>,
    folders: Vec<VfsPath>,
}

#[derive(Default)]
pub(crate) struct SyncState {
    plans: HashMap<u64, Arc<SyncPlan>>,
    scanning: HashSet<u64>,
    pending_deletions: HashMap<u64, PendingDeletion>,
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
        SyncAction::Skip | SyncAction::Delete => return None,
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

fn empty_folder_destination(plan: &SyncPlan, item: &SyncItem, action: SyncAction) -> Option<VfsPath> {
    match action {
        SyncAction::Upload => Some(plan.remote_root.join(&item.path)),
        SyncAction::Download => Some(plan.local_root.join(&item.path)),
        SyncAction::Skip | SyncAction::Delete => None,
    }
}

fn missing_folders(files: &[PlannedFile], empty_folders: &BTreeSet<VfsPath>, root: &VfsPath) -> BTreeSet<VfsPath> {
    let mut folders = BTreeSet::new();
    let below_root = files
        .iter()
        .flat_map(|file| file.destination.ancestors().skip(1))
        .chain(empty_folders.iter().flat_map(|folder| folder.ancestors()));
    for ancestor in below_root {
        if ancestor != *root && ancestor.starts_with(root) {
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
        let (local_rules, remote_rules) = (local.name_rules(), remote.name_rules());
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
                Ok(Some((mut local_scan, mut remote_scan))) => {
                    let mut case_clashes = 0;
                    let mut unstorable = 0;
                    if options.direction != SyncDirection::RemoteToLocal {
                        unstorable += if options.mirror {
                            sync::drop_unstorable_with_counterparts(
                                &mut local_scan.tree,
                                &mut remote_scan.tree,
                                remote_rules,
                            )
                        } else {
                            sync::drop_unstorable(&mut local_scan.tree, remote_rules)
                        };
                        case_clashes +=
                            sync::drop_case_clashes(&mut local_scan.tree, &mut remote_scan.tree, remote_rules);
                    }
                    if options.direction != SyncDirection::LocalToRemote {
                        unstorable += if options.mirror {
                            sync::drop_unstorable_with_counterparts(
                                &mut remote_scan.tree,
                                &mut local_scan.tree,
                                local_rules,
                            )
                        } else {
                            sync::drop_unstorable(&mut remote_scan.tree, local_rules)
                        };
                        case_clashes +=
                            sync::drop_case_clashes(&mut remote_scan.tree, &mut local_scan.tree, local_rules);
                    }
                    let source_tree = match options.direction {
                        SyncDirection::RemoteToLocal => &remote_scan.tree,
                        _ => &local_scan.tree,
                    };
                    let mirror_skipped_empty_source = options.mirror && !sync::has_content(source_tree);
                    TransferEvent::SyncPlanReady {
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
                            case_clashes,
                            unstorable,
                            mirror_skipped_empty_source,
                        },
                    }
                }
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
        if plan.unstorable > 0 {
            let plural = if plan.unstorable == 1 { "" } else { "s" };
            self.report(
                Severity::Warning,
                format!("Skipped {} name{plural} the destination can't store", plan.unstorable),
            );
        }
        if plan.case_clashes > 0 {
            let plural = if plan.case_clashes == 1 { "" } else { "s" };
            self.report(
                Severity::Warning,
                format!(
                    "Skipped {} name{plural} that differ only in letter case from a file on the other side",
                    plan.case_clashes
                ),
            );
        }
        if !plan.unreadable_folders.is_empty() {
            self.report(Severity::Warning, unreadable_folders_notice(&plan.unreadable_folders));
        }
        if plan.mirror_skipped_empty_source {
            self.report(Severity::Warning, "Mirror skipped: the source folder is empty");
        }
        if plan.items.is_empty() {
            if !plan.mirror_skipped_empty_source {
                self.info("Folders are in sync");
            }
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
        let mut delete_files = Vec::new();
        let mut delete_folders = Vec::new();
        let mut upload_folders = BTreeSet::new();
        let mut download_folders = BTreeSet::new();
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
            if action == SyncAction::Delete {
                if item.reason == SyncReason::EmptyFolder {
                    delete_folders.push(item.path.clone());
                } else {
                    delete_files.push(item.path.clone());
                }
                continue;
            }
            if item.reason == SyncReason::EmptyFolder {
                match empty_folder_destination(&plan, item, action) {
                    Some(folder) if action == SyncAction::Upload => upload_folders.insert(folder),
                    Some(folder) => download_folders.insert(folder),
                    None => {
                        refused += 1;
                        false
                    }
                };
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
        let nothing_chosen = uploads.is_empty()
            && downloads.is_empty()
            && upload_folders.is_empty()
            && download_folders.is_empty()
            && delete_files.is_empty()
            && delete_folders.is_empty();
        if nothing_chosen {
            self.info("Nothing to sync");
            return;
        }
        let pending = self.pending_deletion(&plan, delete_files, delete_folders);
        let mut copy_batch = None;
        for (direction, files, folders) in
            [(Direction::Upload, uploads, upload_folders), (Direction::Download, downloads, download_folders)]
        {
            if !files.is_empty() || !folders.is_empty() {
                copy_batch = self.start_sync_batch(&plan, direction, files, &folders);
            }
        }
        match (pending, copy_batch) {
            (Some(pending), Some(batch_id)) => {
                self.sync.pending_deletions.insert(batch_id, pending);
            }
            (Some(pending), None) => self.run_deletions(pending),
            (None, _) => {}
        }
    }

    fn pending_deletion(&self, plan: &SyncPlan, files: Vec<VfsPath>, folders: Vec<VfsPath>) -> Option<PendingDeletion> {
        if files.is_empty() && folders.is_empty() {
            return None;
        }
        let live = self.sessions.get(&plan.session)?;
        let (location, place, target_fs, source_fs, target_root, source_root) = match plan.options.direction {
            SyncDirection::RemoteToLocal => (
                Location::Local,
                THIS_COMPUTER.to_string(),
                self.local_fs.clone(),
                live.fs.clone(),
                plan.local_root.clone(),
                plan.remote_root.clone(),
            ),
            _ => (
                Location::Session(plan.session),
                live.name.clone(),
                live.fs.clone(),
                self.local_fs.clone(),
                plan.remote_root.clone(),
                plan.local_root.clone(),
            ),
        };
        Some(PendingDeletion { location, place, target_fs, source_fs, target_root, source_root, files, folders })
    }

    fn run_deletions(&mut self, pending: PendingDeletion) {
        let events = self.events.clone();
        let reporter = self.reporter.clone();
        let scope = match pending.location {
            Location::Session(id) => Scope::Session(id),
            Location::Local => Scope::Background,
        };
        self.tasks.spawn("sync-delete", scope, move |_| async move {
            let report = delete_mirrored(&pending).await;
            match &report.failure {
                Some((path, err)) => {
                    let message = match pending.location {
                        Location::Local => crate::Message::new(format!("{path}: {err}")),
                        Location::Session(_) => crate::failure_report(format!("Unable to delete {path}"), err),
                    };
                    reporter.report_cause(Severity::Error, message, err);
                }
                None => {
                    if let Some(text) = deleted_summary(&report, &pending.place) {
                        reporter.report(Severity::Info, text);
                    }
                    if report.changed > 0 {
                        reporter.report(
                            Severity::Warning,
                            format!("Left {} alone: it changed since the review", plural(report.changed, "file")),
                        );
                    }
                }
            }
            let _ = events.send(Event::LocationChanged { location: pending.location });
        });
    }

    pub(crate) fn forget_pending_deletions(&mut self, batch_id: u64) {
        self.sync.pending_deletions.remove(&batch_id);
    }

    pub(crate) fn finish_pending_deletions(&mut self, batch_id: u64, copies_clean: bool) {
        let Some(pending) = self.sync.pending_deletions.remove(&batch_id) else {
            return;
        };
        if copies_clean {
            self.run_deletions(pending);
        } else {
            self.report(Severity::Warning, DELETIONS_SKIPPED);
        }
    }

    fn start_sync_batch(
        &mut self, plan: &SyncPlan, direction: Direction, files: Vec<PlannedFile>, empty_folders: &BTreeSet<VfsPath>,
    ) -> Option<u64> {
        let live = self.sessions.get(&plan.session)?;
        let (target_fs, target_root, arrow) = match direction {
            Direction::Upload => (live.fs.clone(), plan.remote_root.clone(), '\u{2191}'),
            Direction::Download => (self.local_fs.clone(), plan.local_root.clone(), '\u{2193}'),
        };
        let rules = target_fs.name_rules();
        let display_name = format!("sync {arrow} {}", target_root);
        let batch_id = self.transfers.start_batch(display_name.clone());
        let session_id = plan.session;
        self.planning.push(PlanningScan { batch_id, session_id, direction, display_name });
        self.sync.scanning.insert(batch_id);
        let folders = missing_folders(&files, empty_folders, &target_root);
        let internal = self.internal.clone();
        self.tasks.spawn("sync-folders", Scope::Planning(batch_id), move |cancel| async move {
            let event = match create_folders(target_fs.as_ref(), &folders, &cancel).await {
                Ok(true) => TransferEvent::SyncFoldersReady {
                    batch_id,
                    session_id,
                    direction,
                    plan: DirectoryPlan {
                        files,
                        skipped_symlinks: 0,
                        taken_names: HashMap::new(),
                        renamed: Vec::new(),
                        name_rules: rules,
                    },
                },
                Ok(false) => TransferEvent::SyncCancelled { batch_id },
                Err(err) => {
                    tracing::debug!("{err:?}");
                    TransferEvent::SyncFailed { batch_id, message: user_message("Sync failed", &err) }
                }
            };
            let _ = internal.send(Internal::Transfer(event));
        });
        Some(batch_id)
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
            self.sync.pending_deletions.remove(&batch_id);
            self.transfers.forget_batch_if_empty(batch_id);
            self.report(Severity::Error, "Sync failed: session disconnected");
            return;
        }
        let questions = crate::transfer::conflicts::conflict_indices(&plan).len();
        let answers = vec![Resolution::Overwrite; questions];
        self.apply_plan_ready(batch_id, session_id, direction, plan, &answers, true);
        if self.transfers.jobs_of(RowKind::Batch(batch_id)).is_empty() {
            self.finish_pending_deletions(batch_id, true);
        }
    }

    pub(crate) fn sync_cancelled(&mut self, batch_id: u64) {
        self.sync.scanning.remove(&batch_id);
        self.finish_pending_deletions(batch_id, false);
        let session = self.planning.iter().find(|scan| scan.batch_id == batch_id).map(|scan| scan.session_id);
        self.clear_planning(batch_id);
        self.transfers.forget_batch_if_empty(batch_id);
        if session.is_some_and(|session| self.sessions.contains_key(&session)) {
            self.info("Sync cancelled");
        }
    }

    pub(crate) fn sync_failed(&mut self, batch_id: u64, message: String) {
        self.sync.scanning.remove(&batch_id);
        self.finish_pending_deletions(batch_id, false);
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

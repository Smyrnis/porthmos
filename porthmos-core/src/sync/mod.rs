use std::collections::{BTreeMap, BTreeSet, HashSet};

use porthmos_vfs::{NameRules, VfsPath};

pub mod scan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    LocalToRemote,
    RemoteToLocal,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBy {
    Time,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncOptions {
    pub direction: SyncDirection,
    pub by: SyncBy,
    pub subfolders: bool,
    pub mirror: bool,
}

impl SyncOptions {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.direction == SyncDirection::Both && self.by == SyncBy::Size {
            return Err("Comparing by size can't decide a direction, so it can't be used with Both");
        }
        if self.mirror && self.direction == SyncDirection::Both {
            return Err("Mirror needs a direction: choose local to remote or remote to local");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    Upload,
    Download,
    Skip,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncReason {
    OnlyLocal,
    OnlyRemote,
    LocalNewer,
    RemoteNewer,
    TargetNewer,
    SameTimeDifferentSize,
    SizeDiffers,
    SizeDiffersTimeUnknown,
    KindMismatch,
    EmptyFolder,
    OnlyOnTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncFacts {
    pub size: u64,
    pub modified: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncItem {
    pub id: u32,
    pub path: VfsPath,
    pub local: Option<SyncFacts>,
    pub remote: Option<SyncFacts>,
    pub action: SyncAction,
    pub reason: SyncReason,
    pub ticked: bool,
    pub flippable: bool,
}

impl SyncItem {
    pub fn allowed_actions(&self, direction: SyncDirection) -> Vec<SyncAction> {
        if self.reason == SyncReason::KindMismatch {
            return vec![SyncAction::Skip];
        }
        let upload = self.local.is_some() && direction != SyncDirection::RemoteToLocal;
        let download = self.remote.is_some() && direction != SyncDirection::LocalToRemote;
        let mut allowed = vec![SyncAction::Skip];
        if self.action == SyncAction::Delete {
            allowed.push(SyncAction::Delete);
        }
        if upload {
            allowed.push(SyncAction::Upload);
        }
        if download {
            allowed.push(SyncAction::Download);
        }
        allowed
    }

    pub fn allows(&self, action: SyncAction, direction: SyncDirection) -> bool {
        self.allowed_actions(direction).contains(&action)
    }

    pub fn next_action(&self, direction: SyncDirection) -> SyncAction {
        self.action_after(self.action, direction)
    }

    pub fn action_after(&self, current: SyncAction, direction: SyncDirection) -> SyncAction {
        let allowed = self.allowed_actions(direction);
        match allowed.iter().position(|action| *action == current) {
            Some(position) => allowed[(position + 1) % allowed.len()],
            None => SyncAction::Skip,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan {
    pub sync_id: u64,
    pub session: u64,
    pub local_root: VfsPath,
    pub remote_root: VfsPath,
    pub options: SyncOptions,
    pub items: Vec<SyncItem>,
    pub skipped_symlinks: usize,
    pub unreadable_folders: Vec<VfsPath>,
    pub case_clashes: usize,
    pub unstorable: usize,
    pub mirror_skipped_empty_source: bool,
}

impl SyncPlan {
    pub fn deletion_counts(&self, choices: &[(u32, SyncAction)]) -> (usize, usize) {
        let mut files = 0;
        let mut folders = 0;
        for (id, action) in choices {
            if *action != SyncAction::Delete {
                continue;
            }
            match self.items.iter().find(|item| item.id == *id) {
                Some(item) if item.reason == SyncReason::EmptyFolder => folders += 1,
                Some(_) => files += 1,
                None => {}
            }
        }
        (files, folders)
    }
}

pub fn mirror_delete_prompt(files: usize, folders: usize, place: &str) -> String {
    let counted = |count: usize, word: &str| {
        if count == 1 { format!("{count} {word}") } else { format!("{count} {word}s") }
    };
    let what = match (files, folders) {
        (files, 0) => counted(files, "file"),
        (0, folders) => counted(folders, "empty folder"),
        (files, folders) => format!("{} and {}", counted(files, "file"), counted(folders, "empty folder")),
    };
    format!("Delete {what} from {place}? Folders left empty are removed too. This can't be undone.")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeKind {
    File,
    Dir,
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeEntry {
    pub kind: TreeKind,
    pub size: u64,
    pub modified: Option<u64>,
}

impl TreeEntry {
    fn facts(self) -> SyncFacts {
        SyncFacts { size: self.size, modified: self.modified }
    }
}

pub type Tree = BTreeMap<VfsPath, TreeEntry>;

fn unstorable_roots(from: &Tree, rules: NameRules) -> BTreeSet<VfsPath> {
    let mut dropped: BTreeSet<VfsPath> = from
        .keys()
        .filter(|path| path.segments().any(|segment| rules.problem(&segment.to_string_lossy()).is_some()))
        .cloned()
        .collect();
    if !rules.is_case_sensitive() {
        let mut by_folded: BTreeMap<String, Vec<&VfsPath>> = BTreeMap::new();
        for path in from.keys() {
            by_folded.entry(rules.fold(&path.to_string_lossy())).or_default().push(path);
        }
        dropped.extend(by_folded.into_values().filter(|group| group.len() > 1).flatten().cloned());
    }
    dropped
        .iter()
        .filter(|path| !path.ancestors().skip(1).any(|ancestor| dropped.contains(&ancestor)))
        .cloned()
        .collect()
}

pub(crate) fn drop_unstorable(from: &mut Tree, rules: NameRules) -> usize {
    let roots = unstorable_roots(from, rules);
    from.retain(|path, _| !path.ancestors().any(|ancestor| roots.contains(&ancestor)));
    roots.len()
}

pub(crate) fn drop_unstorable_with_counterparts(from: &mut Tree, onto: &mut Tree, rules: NameRules) -> usize {
    let roots = unstorable_roots(from, rules);
    from.retain(|path, _| !path.ancestors().any(|ancestor| roots.contains(&ancestor)));
    if !rules.is_case_sensitive() {
        let folded: Vec<String> = roots.iter().map(|root| rules.fold(&root.to_string_lossy())).collect();
        onto.retain(|path, _| {
            let name = rules.fold(&path.to_string_lossy());
            !folded.iter().any(|root| name == *root || name.starts_with(&format!("{root}/")))
        });
    }
    roots.len()
}

pub(crate) fn drop_case_clashes(from: &mut Tree, onto: &mut Tree, onto_rules: NameRules) -> usize {
    if onto_rules.is_case_sensitive() {
        return 0;
    }
    let mut onto_by_folded: BTreeMap<String, Vec<VfsPath>> = BTreeMap::new();
    for path in onto.keys() {
        onto_by_folded.entry(onto_rules.fold(&path.to_string_lossy())).or_default().push(path.clone());
    }
    let mut dropped: Vec<VfsPath> = Vec::new();
    let mut clashes = 0;
    for path in from.keys() {
        let Some(others) = onto_by_folded.get(&onto_rules.fold(&path.to_string_lossy())) else {
            continue;
        };
        let different: Vec<&VfsPath> = others.iter().filter(|other| *other != path).collect();
        if different.is_empty() {
            continue;
        }
        clashes += 1;
        dropped.push(path.clone());
        dropped.extend(different.into_iter().cloned());
    }
    for tree in [from, onto] {
        tree.retain(|path, _| !dropped.iter().any(|gone| path.starts_with(gone)));
    }
    clashes
}

struct Verdict {
    action: SyncAction,
    reason: SyncReason,
    ticked: bool,
}

fn copy(action: SyncAction, reason: SyncReason) -> Option<Verdict> {
    Some(Verdict { action, reason, ticked: true })
}

fn ask(reason: SyncReason) -> Option<Verdict> {
    Some(Verdict { action: SyncAction::Skip, reason, ticked: false })
}

fn only_one_side(on_local: bool, direction: SyncDirection) -> Option<Verdict> {
    match (on_local, direction) {
        (true, SyncDirection::LocalToRemote | SyncDirection::Both) => copy(SyncAction::Upload, SyncReason::OnlyLocal),
        (false, SyncDirection::RemoteToLocal | SyncDirection::Both) => {
            copy(SyncAction::Download, SyncReason::OnlyRemote)
        }
        _ => None,
    }
}

fn empty_folder_on_one_side(on_local: bool, direction: SyncDirection) -> Option<Verdict> {
    only_one_side(on_local, direction).map(|verdict| Verdict { reason: SyncReason::EmptyFolder, ..verdict })
}

fn mirror_verdict(target_entry: &TreeEntry, path: &VfsPath, target_holds: &HashSet<VfsPath>) -> Option<Verdict> {
    let reason = match target_entry.kind {
        TreeKind::File => SyncReason::OnlyOnTarget,
        TreeKind::Dir if !target_holds.contains(path) => SyncReason::EmptyFolder,
        _ => return None,
    };
    Some(Verdict { action: SyncAction::Delete, reason, ticked: false })
}

pub(crate) fn has_content(tree: &Tree) -> bool {
    tree.values().any(|entry| matches!(entry.kind, TreeKind::File | TreeKind::Dir))
}

fn folders_holding_anything(tree: &Tree) -> HashSet<VfsPath> {
    tree.keys().filter_map(|path| path.parent()).collect()
}

fn folders_holding_something(tree: &Tree) -> HashSet<VfsPath> {
    tree.iter()
        .filter(|(_, entry)| matches!(entry.kind, TreeKind::File | TreeKind::Dir))
        .filter_map(|(path, _)| path.parent())
        .collect()
}

fn newer_side(local_is_newer: bool, direction: SyncDirection) -> Option<Verdict> {
    match (local_is_newer, direction) {
        (true, SyncDirection::LocalToRemote | SyncDirection::Both) => copy(SyncAction::Upload, SyncReason::LocalNewer),
        (false, SyncDirection::RemoteToLocal | SyncDirection::Both) => {
            copy(SyncAction::Download, SyncReason::RemoteNewer)
        }
        _ => ask(SyncReason::TargetNewer),
    }
}

fn by_size(local: TreeEntry, remote: TreeEntry, direction: SyncDirection) -> Option<Verdict> {
    if local.size == remote.size {
        return None;
    }
    match direction {
        SyncDirection::LocalToRemote => copy(SyncAction::Upload, SyncReason::SizeDiffers),
        SyncDirection::RemoteToLocal => copy(SyncAction::Download, SyncReason::SizeDiffers),
        SyncDirection::Both => ask(SyncReason::SizeDiffers),
    }
}

fn by_time(
    local: TreeEntry, remote: TreeEntry, direction: SyncDirection, tolerance: u64, target_keeps_times: bool,
) -> Option<Verdict> {
    let sizes_equal = local.size == remote.size;
    let (Some(local_time), Some(remote_time)) = (local.modified, remote.modified) else {
        if sizes_equal {
            return None;
        }
        return match direction {
            SyncDirection::LocalToRemote => copy(SyncAction::Upload, SyncReason::SizeDiffersTimeUnknown),
            SyncDirection::RemoteToLocal => copy(SyncAction::Download, SyncReason::SizeDiffersTimeUnknown),
            SyncDirection::Both => ask(SyncReason::SizeDiffersTimeUnknown),
        };
    };
    let verdict = if local_time > remote_time.saturating_add(tolerance) {
        newer_side(true, direction)
    } else if remote_time > local_time.saturating_add(tolerance) {
        newer_side(false, direction)
    } else {
        return if sizes_equal { None } else { ask(SyncReason::SameTimeDifferentSize) };
    };
    match verdict {
        Some(found) if found.reason == SyncReason::TargetNewer && sizes_equal && !target_keeps_times => None,
        other => other,
    }
}

fn flippable(verdict: &Verdict) -> bool {
    verdict.action == SyncAction::Skip
        && matches!(
            verdict.reason,
            SyncReason::TargetNewer | SyncReason::SameTimeDifferentSize | SyncReason::SizeDiffersTimeUnknown
        )
}

pub fn compare(
    local: &Tree, remote: &Tree, options: SyncOptions, tolerance: u64, target_keeps_times: bool,
) -> Vec<SyncItem> {
    if options.validate().is_err() {
        return Vec::new();
    }
    let paths: BTreeSet<&VfsPath> = local.keys().chain(remote.keys()).collect();
    let (local_holds, remote_holds) = (folders_holding_something(local), folders_holding_something(remote));
    let mirror_target_is_local = match (options.mirror, options.direction) {
        (true, SyncDirection::LocalToRemote) if has_content(local) => Some(false),
        (true, SyncDirection::RemoteToLocal) if has_content(remote) => Some(true),
        _ => None,
    };
    let target_tree = mirror_target_is_local.map(|is_local| if is_local { local } else { remote });
    let target_holds = target_tree.map(folders_holding_anything).unwrap_or_default();
    let mut mismatched: HashSet<VfsPath> = HashSet::new();
    let mut items = Vec::new();
    for path in paths {
        if path.ancestors().skip(1).any(|ancestor| mismatched.contains(&ancestor)) {
            continue;
        }
        let verdict = match (local.get(path), remote.get(path)) {
            (Some(local_entry), Some(remote_entry)) if local_entry.kind != remote_entry.kind => {
                mismatched.insert(path.clone());
                ask(SyncReason::KindMismatch)
            }
            (Some(local_entry), Some(remote_entry)) if local_entry.kind == TreeKind::File => match options.by {
                SyncBy::Size => by_size(*local_entry, *remote_entry, options.direction),
                SyncBy::Time => by_time(*local_entry, *remote_entry, options.direction, tolerance, target_keeps_times),
            },
            (Some(entry), None) if entry.kind == TreeKind::File => only_one_side(true, options.direction),
            (None, Some(entry)) if entry.kind == TreeKind::File => only_one_side(false, options.direction),
            (Some(entry), None) if entry.kind == TreeKind::Dir && !local_holds.contains(path) => {
                empty_folder_on_one_side(true, options.direction)
            }
            (None, Some(entry)) if entry.kind == TreeKind::Dir && !remote_holds.contains(path) => {
                empty_folder_on_one_side(false, options.direction)
            }
            _ => None,
        };
        let verdict = verdict.or_else(|| {
            let is_local = mirror_target_is_local?;
            let (target, source) = if is_local { (local, remote) } else { (remote, local) };
            match (target.get(path), source.get(path)) {
                (Some(entry), None) => mirror_verdict(entry, path, &target_holds),
                _ => None,
            }
        });
        let Some(verdict) = verdict else {
            continue;
        };
        let can_flip = flippable(&verdict);
        items.push(SyncItem {
            id: items.len() as u32,
            path: path.clone(),
            local: local.get(path).map(|entry| entry.facts()),
            remote: remote.get(path).map(|entry| entry.facts()),
            action: verdict.action,
            reason: verdict.reason,
            ticked: verdict.ticked,
            flippable: can_flip,
        });
    }
    items
}

#[cfg(test)]
mod tests;

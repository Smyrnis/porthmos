use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
};

use futures_util::future::BoxFuture;
use porthmos_vfs::{Entry, ErrorKind, FileKind, FileSystem, Metadata, PART_SUFFIX, ProtocolError, VfsPath};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExistingFile {
    pub size: u64,
    pub modified: Option<u64>,
    pub is_dir: bool,
}

impl From<Metadata> for ExistingFile {
    fn from(metadata: Metadata) -> Self {
        Self { size: metadata.size, modified: metadata.modified, is_dir: metadata.is_dir() }
    }
}

pub type DestinationListing = HashMap<String, ExistingFile>;

pub struct PlannedFile {
    pub source: VfsPath,
    pub destination: VfsPath,
    pub display_name: String,
    pub size: u64,
    pub existing: Option<ExistingFile>,
    pub source_modified: Option<u64>,
    pub partial: Option<ExistingFile>,
    pub resume: bool,
}

impl PlannedFile {
    pub fn is_conflict(&self) -> bool {
        self.existing.is_some() || self.partial.is_some()
    }
}

pub struct DirectoryPlan {
    pub files: Vec<PlannedFile>,
    pub skipped_symlinks: usize,
    pub taken_names: HashMap<VfsPath, HashSet<String>>,
}

pub enum PlanOutcome {
    Ready(DirectoryPlan),
    Cancelled,
}

enum Walk {
    Completed,
    Cancelled,
}

pub(crate) struct DiscoveredTree {
    pub(crate) directories: Vec<VfsPath>,
    pub(crate) files: Vec<(VfsPath, u64, Option<u64>)>,
    pub(crate) skipped_symlinks: usize,
    pub(crate) symlinks: Vec<VfsPath>,
    pub(crate) unreadable: Vec<VfsPath>,
}

async fn discover_tree(
    fs: &dyn FileSystem, root: &VfsPath, cancel: &AtomicBool,
) -> Result<Option<DiscoveredTree>, ProtocolError> {
    discover(fs, root, true, cancel).await
}

pub(crate) async fn discover(
    fs: &dyn FileSystem, root: &VfsPath, recursive: bool, cancel: &AtomicBool,
) -> Result<Option<DiscoveredTree>, ProtocolError> {
    discover_with(fs, root, recursive, Strictness::Strict, cancel).await
}

pub(crate) async fn discover_skipping_unreadable(
    fs: &dyn FileSystem, root: &VfsPath, recursive: bool, cancel: &AtomicBool,
) -> Result<Option<DiscoveredTree>, ProtocolError> {
    discover_with(fs, root, recursive, Strictness::SkipUnreadableFolders, cancel).await
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Strictness {
    Strict,
    SkipUnreadableFolders,
}

async fn discover_with(
    fs: &dyn FileSystem, root: &VfsPath, recursive: bool, strictness: Strictness, cancel: &AtomicBool,
) -> Result<Option<DiscoveredTree>, ProtocolError> {
    let mut tree = DiscoveredTree {
        directories: Vec::new(),
        files: Vec::new(),
        skipped_symlinks: 0,
        symlinks: Vec::new(),
        unreadable: Vec::new(),
    };
    match discover_tree_into(fs, root, &VfsPath::new(""), recursive, strictness, &mut tree, cancel).await? {
        Walk::Completed => Ok(Some(tree)),
        Walk::Cancelled => Ok(None),
    }
}

fn discover_tree_into<'a>(
    fs: &'a dyn FileSystem, root: &'a VfsPath, relative: &'a VfsPath, recursive: bool, strictness: Strictness,
    tree: &'a mut DiscoveredTree, cancel: &'a AtomicBool,
) -> BoxFuture<'a, Result<Walk, ProtocolError>> {
    Box::pin(async move {
        let items = match fs.read_dir(&root.join(relative)).await {
            Ok(items) => items,
            Err(err)
                if strictness == Strictness::SkipUnreadableFolders
                    && err.kind() == ErrorKind::PermissionDenied
                    && !relative.is_empty() =>
            {
                tree.unreadable.push(relative.clone());
                return Ok(Walk::Completed);
            }
            Err(err) => return Err(err),
        };
        for item in items {
            if cancel.load(Ordering::Relaxed) {
                return Ok(Walk::Cancelled);
            }
            let entry_relative = relative.join(&item.name);

            match item.metadata.kind {
                FileKind::Symlink => {
                    tree.skipped_symlinks += 1;
                    tree.symlinks.push(entry_relative);
                }
                FileKind::Dir => {
                    tree.directories.push(entry_relative.clone());
                    if recursive
                        && let Walk::Cancelled =
                            discover_tree_into(fs, root, &entry_relative, recursive, strictness, tree, cancel).await?
                    {
                        return Ok(Walk::Cancelled);
                    }
                }
                FileKind::File => tree.files.push((entry_relative, item.metadata.size, item.metadata.modified)),
            }
        }
        Ok(Walk::Completed)
    })
}

pub(crate) async fn ensure_directory(fs: &dyn FileSystem, path: &VfsPath) -> Result<bool, ProtocolError> {
    if fs.stat(path).await.is_ok_and(|metadata| metadata.is_dir()) {
        return Ok(false);
    }
    fs.create_dir(path).await?;
    Ok(true)
}

fn planned_file_for_loose_entry(entry: &Entry, dest_dir: &VfsPath) -> PlannedFile {
    PlannedFile {
        source: entry.path.clone(),
        destination: dest_dir.join(&entry.name),
        display_name: entry.name.clone(),
        size: entry.size,
        existing: None,
        source_modified: entry.modified,
        partial: None,
        resume: false,
    }
}

fn planned_files_for_tree(entry: &Entry, dest_root: &VfsPath, tree: &DiscoveredTree) -> Vec<PlannedFile> {
    tree.files
        .iter()
        .map(|(relative_file, size, modified)| PlannedFile {
            source: entry.path.join(relative_file),
            destination: dest_root.join(relative_file),
            display_name: relative_file.to_string_lossy().into_owned(),
            size: *size,
            existing: None,
            source_modified: *modified,
            partial: None,
            resume: false,
        })
        .collect()
}

pub async fn plan_copy(
    source: &dyn FileSystem, destination: &dyn FileSystem, source_entries: Vec<Entry>, dest_dir: &VfsPath,
    cancel: &AtomicBool,
) -> Result<PlanOutcome, ProtocolError> {
    let mut files = Vec::new();
    let mut skipped_symlinks = 0;
    let mut created: HashSet<VfsPath> = HashSet::new();

    for entry in source_entries {
        if cancel.load(Ordering::Relaxed) {
            return Ok(PlanOutcome::Cancelled);
        }
        if !entry.is_dir {
            files.push(planned_file_for_loose_entry(&entry, dest_dir));
            continue;
        }

        let Some(tree) = discover_tree(source, &entry.path, cancel).await? else {
            return Ok(PlanOutcome::Cancelled);
        };
        skipped_symlinks += tree.skipped_symlinks;

        let dest_root = dest_dir.join(&entry.name);
        let directories =
            std::iter::once(dest_root.clone()).chain(tree.directories.iter().map(|relative| dest_root.join(relative)));
        for directory in directories {
            if cancel.load(Ordering::Relaxed) {
                return Ok(PlanOutcome::Cancelled);
            }
            if ensure_directory(destination, &directory).await? {
                created.insert(directory);
            }
        }

        files.extend(planned_files_for_tree(&entry, &dest_root, &tree));
    }

    let names_by_parent = names_by_parent(&files);
    let mut listings = HashMap::new();
    for parent in parents_needing_listing(&files, &created) {
        if cancel.load(Ordering::Relaxed) {
            return Ok(PlanOutcome::Cancelled);
        }
        let planned_names = names_by_parent.get(&parent).map(Vec::as_slice).unwrap_or_default();
        let listing = list_destination(destination, &parent, planned_names).await?;
        listings.insert(parent, listing);
    }
    mark_conflicts(&mut files, &listings);
    for file in files.iter_mut().filter(|file| file.is_conflict() && file.source_modified.is_none()) {
        file.source_modified = source.stat(&file.source).await.ok().and_then(|metadata| metadata.modified);
    }
    let taken_names = taken_names(&files, &listings);

    Ok(PlanOutcome::Ready(DirectoryPlan { files, skipped_symlinks, taken_names }))
}

fn destination_parent_and_name(file: &PlannedFile) -> Option<(VfsPath, String)> {
    Some((file.destination.parent()?, file.destination.file_name()?.to_string_lossy().into_owned()))
}

pub(crate) fn parents_needing_listing(files: &[PlannedFile], created: &HashSet<VfsPath>) -> Vec<VfsPath> {
    let mut parents: Vec<VfsPath> = files
        .iter()
        .filter_map(destination_parent_and_name)
        .map(|(parent, _)| parent)
        .filter(|parent| !created.contains(parent))
        .collect();
    parents.sort();
    parents.dedup();
    parents
}

fn names_by_parent(files: &[PlannedFile]) -> HashMap<VfsPath, Vec<String>> {
    let mut names: HashMap<VfsPath, Vec<String>> = HashMap::new();
    for (parent, name) in files.iter().filter_map(destination_parent_and_name) {
        names.entry(parent).or_default().push(name);
    }
    names
}

pub(crate) fn mark_conflicts(files: &mut [PlannedFile], listings: &HashMap<VfsPath, DestinationListing>) {
    for file in files.iter_mut() {
        let Some((parent, name)) = destination_parent_and_name(file) else {
            continue;
        };
        file.existing = listings.get(&parent).and_then(|listing| listing.get(&name)).copied();
        file.partial = listings
            .get(&parent)
            .and_then(|listing| listing.get(&format!("{name}{PART_SUFFIX}")))
            .copied()
            .filter(|partial| !partial.is_dir);
    }
}

pub(crate) fn taken_names(
    files: &[PlannedFile], listings: &HashMap<VfsPath, DestinationListing>,
) -> HashMap<VfsPath, HashSet<String>> {
    let mut taken: HashMap<VfsPath, HashSet<String>> = HashMap::new();
    for (parent, listing) in listings {
        taken.entry(parent.clone()).or_default().extend(listing.keys().cloned());
    }
    for (parent, name) in files.iter().filter_map(destination_parent_and_name) {
        taken.entry(parent).or_default().insert(name);
    }
    taken
}

pub(crate) async fn list_destination(
    fs: &dyn FileSystem, dir: &VfsPath, planned_names: &[String],
) -> Result<DestinationListing, ProtocolError> {
    let mut listing = HashMap::new();
    match fs.read_dir(dir).await {
        Ok(items) => {
            for item in items {
                let existing = if item.metadata.kind == FileKind::Symlink {
                    fs.stat(&item.path).await.unwrap_or(item.metadata)
                } else {
                    item.metadata
                };
                listing.insert(item.name, ExistingFile::from(existing));
            }
        }
        Err(_) if fs.stat(dir).await.is_err() => {}
        Err(_) => {
            for name in planned_names {
                if let Ok(metadata) = fs.stat(&dir.join(name)).await {
                    listing.insert(name.clone(), ExistingFile::from(metadata));
                }
            }
        }
    }
    Ok(listing)
}

#[cfg(test)]
mod tests;

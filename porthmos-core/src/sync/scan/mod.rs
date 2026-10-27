use std::sync::atomic::AtomicBool;

use porthmos_vfs::{ErrorKind, FileSystem, PART_SUFFIX, ProtocolError, VfsPath};

use super::{Tree, TreeEntry, TreeKind};
use crate::transfer::plan::{DiscoveredTree, discover_skipping_unreadable};

const PARENT: &str = "..";

pub struct Scanned {
    pub tree: Tree,
    pub skipped_symlinks: usize,
    pub unreadable: Vec<VfsPath>,
}

fn check_relative(path: &VfsPath) -> Result<(), ProtocolError> {
    let plain = !path.is_absolute() && !path.is_empty() && path.segments().all(|segment| segment != PARENT);
    if plain {
        return Ok(());
    }
    Err(ProtocolError::new(
        ErrorKind::Other,
        anyhow::anyhow!(
            "The listing contains the path {:?}, which would lead outside the chosen folder",
            path.to_string_lossy()
        ),
    ))
}

fn check_parent_is_a_folder(tree: &Tree, path: &VfsPath) -> Result<(), ProtocolError> {
    let Some(parent) = path.parent().filter(|parent| !parent.is_empty()) else {
        return Ok(());
    };
    if tree.get(&parent).is_some_and(|entry| entry.kind == TreeKind::Dir) {
        return Ok(());
    }
    Err(ProtocolError::new(
        ErrorKind::Other,
        anyhow::anyhow!(
            "The listing contains the path {:?}, but not the folder it should be in",
            path.to_string_lossy()
        ),
    ))
}

fn check_depth(path: &VfsPath, subfolders: bool) -> Result<(), ProtocolError> {
    if subfolders || path.segments().count() <= 1 {
        return Ok(());
    }
    Err(ProtocolError::new(
        ErrorKind::Other,
        anyhow::anyhow!(
            "The listing contains the nested path {:?} although subfolders are off",
            path.to_string_lossy()
        ),
    ))
}

fn insert_once(tree: &mut Tree, path: VfsPath, entry: TreeEntry) -> Result<(), ProtocolError> {
    if tree.contains_key(&path) {
        return Err(ProtocolError::new(
            ErrorKind::Other,
            anyhow::anyhow!("The listing contains the path {:?} more than once", path.to_string_lossy()),
        ));
    }
    tree.insert(path, entry);
    Ok(())
}

fn build_tree(found: DiscoveredTree, subfolders: bool) -> Result<Scanned, ProtocolError> {
    let mut tree = Tree::new();
    for directory in found.directories {
        check_relative(&directory)?;
        insert_once(&mut tree, directory, TreeEntry { kind: TreeKind::Dir, size: 0, modified: None })?;
    }
    for (path, size, modified) in found.files {
        check_relative(&path)?;
        if path.to_string_lossy().ends_with(PART_SUFFIX) {
            continue;
        }
        insert_once(&mut tree, path, TreeEntry { kind: TreeKind::File, size, modified })?;
    }
    for path in found.symlinks {
        check_relative(&path)?;
        insert_once(&mut tree, path, TreeEntry { kind: TreeKind::Symlink, size: 0, modified: None })?;
    }
    for path in tree.keys() {
        check_depth(path, subfolders)?;
        check_parent_is_a_folder(&tree, path)?;
    }
    Ok(Scanned { tree, skipped_symlinks: found.skipped_symlinks, unreadable: found.unreadable })
}

pub async fn scan_tree(
    fs: &dyn FileSystem, root: &VfsPath, subfolders: bool, cancel: &AtomicBool,
) -> Result<Option<Scanned>, ProtocolError> {
    let found = discover_skipping_unreadable(fs, root, subfolders, cancel)
        .await
        .map_err(|err| ProtocolError::new(err.kind(), anyhow::anyhow!("{}: {err}", root)))?;
    let Some(found) = found else {
        return Ok(None);
    };
    build_tree(found, subfolders).map(Some)
}

fn leave_out_unreadable_folders(mut left: Scanned, mut right: Scanned) -> (Scanned, Scanned) {
    let mut unreadable: Vec<VfsPath> = left.unreadable.iter().chain(&right.unreadable).cloned().collect();
    unreadable.sort();
    unreadable.dedup();
    for scanned in [&mut left, &mut right] {
        scanned.tree.retain(|path, _| !unreadable.iter().any(|folder| path.starts_with(folder)));
        scanned.unreadable = unreadable.clone();
    }
    (left, right)
}

pub async fn scan_both(
    local: &dyn FileSystem, local_root: &VfsPath, remote: &dyn FileSystem, remote_root: &VfsPath, subfolders: bool,
    cancel: &AtomicBool,
) -> Result<Option<(Scanned, Scanned)>, ProtocolError> {
    let (local_scan, remote_scan) = tokio::try_join!(
        scan_tree(local, local_root, subfolders, cancel),
        scan_tree(remote, remote_root, subfolders, cancel)
    )?;
    Ok(match (local_scan, remote_scan) {
        (Some(left), Some(right)) => Some(leave_out_unreadable_folders(left, right)),
        _ => None,
    })
}

#[cfg(test)]
mod tests;

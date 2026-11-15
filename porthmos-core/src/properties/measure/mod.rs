use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use porthmos_vfs::{DirItem, ErrorKind, FileKind, FileSystem, Metadata, ProtocolError, VfsPath};

use super::PROGRESS_INTERVAL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Measured {
    pub bytes: u64,
    pub files: u64,
    pub folders: u64,
    pub symlinks: u64,
    pub unreadable: u64,
}

impl Measured {
    fn add_file(&mut self, size: u64) {
        self.files += 1;
        self.bytes = self.bytes.saturating_add(size);
    }
}

async fn kind_of_root(
    fs: &dyn FileSystem, root: &VfsPath, listings: &mut HashMap<VfsPath, Option<Vec<DirItem>>>,
) -> Result<Metadata, ProtocolError> {
    if let Some(parent) = root.parent().filter(|_| root.file_name().is_some()) {
        if !listings.contains_key(&parent) {
            listings.insert(parent.clone(), fs.read_dir(&parent).await.ok());
        }
        let listed = listings
            .get(&parent)
            .and_then(Option::as_ref)
            .and_then(|items| items.iter().find(|item| item.path == *root).map(|item| item.metadata));
        if let Some(metadata) = listed {
            return Ok(metadata);
        }
    }
    fs.stat(root).await
}

pub async fn measure(
    fs: &dyn FileSystem, roots: &[VfsPath], cancel: &AtomicBool, progress: &mut (dyn FnMut(&Measured) + Send),
) -> Result<Option<Measured>, ProtocolError> {
    measure_with_interval(fs, roots, cancel, progress, PROGRESS_INTERVAL).await
}

pub(crate) async fn measure_with_interval(
    fs: &dyn FileSystem, roots: &[VfsPath], cancel: &AtomicBool, progress: &mut (dyn FnMut(&Measured) + Send),
    interval: std::time::Duration,
) -> Result<Option<Measured>, ProtocolError> {
    let mut total = Measured::default();
    let mut pending: Vec<VfsPath> = Vec::new();
    let mut listings: HashMap<VfsPath, Option<Vec<DirItem>>> = HashMap::new();
    for root in roots {
        let metadata = kind_of_root(fs, root, &mut listings).await?;
        match metadata.kind {
            FileKind::Dir => pending.push(root.clone()),
            FileKind::File => total.add_file(metadata.size),
            FileKind::Symlink => total.symlinks += 1,
        }
    }
    let mut last_report = Instant::now();
    while let Some(folder) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        match fs.read_dir(&folder).await {
            Ok(items) => {
                for item in items {
                    match item.metadata.kind {
                        FileKind::Dir => {
                            total.folders += 1;
                            pending.push(folder.join(&item.name));
                        }
                        FileKind::File => total.add_file(item.metadata.size),
                        FileKind::Symlink => total.symlinks += 1,
                    }
                }
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => total.unreadable += 1,
            Err(err) => return Err(err),
        }
        if last_report.elapsed() >= interval {
            progress(&total);
            last_report = Instant::now();
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    progress(&total);
    Ok(Some(total))
}

#[cfg(test)]
mod tests;

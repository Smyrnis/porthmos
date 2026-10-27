use std::{
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

use porthmos_vfs::VfsPath;

pub fn to_os_path(path: &VfsPath) -> PathBuf {
    let mut os_path = if path.is_absolute() { PathBuf::from("/") } else { PathBuf::new() };
    os_path.extend(path.segments());
    os_path
}

pub fn from_os_path(path: &Path) -> VfsPath {
    let segments = path.components().filter_map(|component| match component {
        Component::Normal(name) => Some(name),
        Component::ParentDir => Some(OsStr::new("..")),
        Component::RootDir | Component::CurDir | Component::Prefix(_) => None,
    });
    VfsPath::from_segments(path.has_root(), segments)
}

#[cfg(test)]
mod tests;

use std::{io, path::PathBuf};

use porthmos_vfs::VfsPath;

#[cfg(any(windows, test))]
mod problem;

#[cfg(any(windows, test))]
pub(crate) use problem::path_problem;

#[cfg(unix)]
pub fn checked_os_path(path: &VfsPath) -> io::Result<PathBuf> {
    Ok(super::to_os_path(path))
}

#[cfg(windows)]
pub fn checked_os_path(path: &VfsPath) -> io::Result<PathBuf> {
    match path_problem(path) {
        Some(message) => Err(io::Error::new(io::ErrorKind::InvalidInput, message)),
        None => Ok(super::to_os_path(path)),
    }
}

use std::{ffi::OsString, path::Path};

use porthmos_lfs::from_os_path;
use porthmos_vfs::VfsPath;

use crate::Location;

const PARENT: &str = "..";

fn home_remainder(text: &str) -> Option<&str> {
    let remainder = text.strip_prefix('~')?;
    (remainder.is_empty() || remainder.starts_with('/')).then_some(remainder)
}

pub(crate) fn needs_home(text: &str) -> bool {
    home_remainder(text.trim()).is_some()
}

fn parsed(location: Location, text: &str) -> VfsPath {
    match location {
        Location::Local => from_os_path(Path::new(text)),
        Location::Session(_) => VfsPath::new(text),
    }
}

fn without_dot_dot(path: &VfsPath) -> VfsPath {
    let mut kept: Vec<OsString> = Vec::new();
    for segment in path.segments() {
        if segment == PARENT {
            kept.pop();
        } else {
            kept.push(segment.to_os_string());
        }
    }
    VfsPath::from_segments(path.is_absolute(), kept)
}

pub(crate) fn resolve_path_text(location: Location, text: &str, from: &VfsPath, home: &VfsPath) -> VfsPath {
    let text = text.trim();
    let target = match home_remainder(text) {
        Some(remainder) => home.join(VfsPath::new(remainder.trim_start_matches('/'))),
        None => from.join(parsed(location, text)),
    };
    without_dot_dot(&target)
}

#[cfg(test)]
mod tests;

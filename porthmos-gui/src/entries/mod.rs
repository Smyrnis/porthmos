use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use porthmos_core::{Entry, from_os_path};

fn entry_of(path: &Path) -> Result<Entry, String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{}: has no name to copy", path.display()))?;
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let modified =
        metadata.modified().ok().and_then(|moment| moment.duration_since(UNIX_EPOCH).ok()).map(|age| age.as_secs());
    Ok(Entry {
        hidden: name.starts_with('.'),
        name,
        path: from_os_path(path),
        is_dir: metadata.is_dir(),
        size: if metadata.is_dir() { 0 } else { metadata.len() },
        modified,
        ..Entry::default()
    })
}

pub fn entries_from_paths(paths: &[PathBuf]) -> (Vec<Entry>, Vec<String>) {
    let mut entries = Vec::with_capacity(paths.len());
    let mut problems = Vec::new();
    for path in paths {
        match entry_of(path) {
            Ok(entry) => entries.push(entry),
            Err(problem) => problems.push(problem),
        }
    }
    (entries, problems)
}

#[cfg(test)]
mod tests;

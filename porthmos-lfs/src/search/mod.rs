use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use porthmos_vfs::{Entry, FileKind, Glob, SearchEvent, SearchSender};

use crate::{explain, from_os_path, metadata_from};

pub(crate) fn search_tree(
    root: &Path, pattern: &str, max_depth: usize, max_results: usize, tx: &SearchSender, cancel: &AtomicBool,
) {
    let glob = Glob::new(pattern);
    let mut found = 0usize;
    let mut truncated = false;
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    let mut is_root = true;
    let mut incomplete = false;

    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if depth > max_depth {
            continue;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) => {
                if is_root {
                    let _ = tx.send(SearchEvent::Failed(explain(err).to_string()));
                    return;
                }
                incomplete = true;
                continue;
            }
        };
        is_root = false;

        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let Ok(entry) = entry else {
                continue;
            };
            if found >= max_results {
                truncated = true;
                break;
            }

            let name = entry.file_name();
            let name = name.to_string_lossy();
            if glob.matches(&name) {
                let path = entry.path();
                if let Ok(raw) = fs::symlink_metadata(&path) {
                    let metadata = metadata_from(&raw);
                    let found_entry = Entry {
                        name: name.into_owned(),
                        path: from_os_path(&path),
                        is_dir: metadata.kind == FileKind::Dir,
                        size: metadata.size,
                        permissions: None,
                        modified: metadata.modified,
                        ..Default::default()
                    };
                    if tx.send(SearchEvent::Found(found_entry)).is_err() {
                        return;
                    }
                    found += 1;
                }
            }

            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                stack.push((entry.path(), depth + 1));
            }
        }

        if found >= max_results {
            truncated = true;
            break;
        }
    }

    let _ = tx.send(SearchEvent::Done { truncated, incomplete });
}

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, Ordering};

use porthmos_vfs::{ErrorKind, FileKind, FileSystem, ProtocolError, VfsPath};

pub const COUNT_LIMIT: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContentCount {
    pub files: u64,
    pub folders: u64,
    pub unreadable: u64,
    pub capped: bool,
}

impl ContentCount {
    fn items(&self) -> u64 {
        self.files + self.folders
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contents {
    Counting,
    Counted(ContentCount),
    Unavailable,
}

pub async fn count_contents(
    fs: &dyn FileSystem, folders: &[VfsPath], limit: u64, cancel: &AtomicBool,
) -> Result<Option<ContentCount>, ProtocolError> {
    let mut count = ContentCount::default();
    let mut pending: Vec<VfsPath> = folders.iter().rev().cloned().collect();
    while let Some(folder) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let items = match fs.read_dir(&folder).await {
            Ok(items) => items,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                count.unreadable += 1;
                continue;
            }
            Err(err) => return Err(err),
        };
        for item in items {
            match item.metadata.kind {
                FileKind::Dir => {
                    count.folders += 1;
                    pending.push(folder.join(&item.name));
                }
                FileKind::File | FileKind::Symlink => count.files += 1,
            }
            if count.items() > limit {
                count.capped = true;
                return Ok(Some(count));
            }
        }
    }
    Ok(Some(count))
}

fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let first = digits.len() % 3;
    let mut groups: Vec<&str> = Vec::new();
    if first > 0 {
        groups.push(&digits[..first]);
    }
    groups.extend((first..digits.len()).step_by(3).map(|start| &digits[start..start + 3]));
    groups.join(",")
}

fn counted(number: u64, singular: &str, plural: &str) -> Option<String> {
    match number {
        0 => None,
        1 => Some(format!("1 {singular}")),
        _ => Some(format!("{} {plural}", grouped(number))),
    }
}

pub fn contents_line(contents: &Contents) -> String {
    let count = match contents {
        Contents::Counting => return "Counting what is inside\u{2026}".to_string(),
        Contents::Unavailable => return "Couldn't count what is inside.".to_string(),
        Contents::Counted(count) => count,
    };
    if count.capped {
        return format!("Inside: more than {} items.", grouped(COUNT_LIMIT));
    }
    let parts: Vec<String> = [counted(count.files, "file", "files"), counted(count.folders, "folder", "folders")]
        .into_iter()
        .flatten()
        .collect();
    let mut text = String::from("Inside: ");
    match (parts.is_empty(), count.unreadable > 0) {
        (true, true) => text.push_str("nothing found"),
        (true, false) => text.push_str("nothing"),
        (false, true) => text.push_str(&format!("at least {}", parts.join(" and "))),
        (false, false) => text.push_str(&parts.join(" and ")),
    }
    match count.unreadable {
        0 => text.push('.'),
        1 => text.push_str("; 1 folder could not be read."),
        more => text.push_str(&format!("; {more} folders could not be read.")),
    }
    text
}

#[cfg(test)]
mod tests;

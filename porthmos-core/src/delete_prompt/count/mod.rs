use crate::i18n;
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

pub(crate) fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let first = digits.len() % 3;
    let mut groups: Vec<&str> = Vec::new();
    if first > 0 {
        groups.push(&digits[..first]);
    }
    groups.extend((first..digits.len()).step_by(3).map(|start| &digits[start..start + 3]));
    groups.join(",")
}

pub(crate) fn counted(number: u64, key: &str) -> Option<String> {
    if number == 0 {
        return None;
    }
    let form = if number == 1 { "one" } else { "other" };
    Some(i18n::text_with(&format!("{key}.{form}"), &[("count", &grouped(number))]))
}

pub(crate) fn joined_and(parts: Vec<String>) -> String {
    match parts.as_slice() {
        [first, second] => i18n::text_with("common.and", &[("first", first), ("second", second)]),
        _ => parts.join(", "),
    }
}

pub fn contents_line(contents: &Contents) -> String {
    let count = match contents {
        Contents::Counting => return i18n::text("contents.counting"),
        Contents::Unavailable => return i18n::text("contents.unavailable"),
        Contents::Counted(count) => count,
    };
    if count.capped {
        return i18n::text_with("contents.capped", &[("count", &grouped(COUNT_LIMIT))]);
    }
    let parts: Vec<String> =
        [counted(count.files, "count.files"), counted(count.folders, "count.folders")].into_iter().flatten().collect();
    let what = joined_and(parts.clone());
    let mut text = match (parts.is_empty(), count.unreadable > 0) {
        (true, true) => i18n::text("contents.nothing_found"),
        (true, false) => i18n::text("contents.nothing"),
        (false, true) => i18n::text_with("contents.at_least", &[("what", &what)]),
        (false, false) => i18n::text_with("contents.some", &[("what", &what)]),
    };
    match count.unreadable {
        0 => text.push('.'),
        more => text.push_str(&i18n::plural("contents.unreadable", more as usize)),
    }
    text
}

#[cfg(test)]
mod tests;

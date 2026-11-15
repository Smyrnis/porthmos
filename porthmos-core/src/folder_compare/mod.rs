use std::collections::HashMap;

use porthmos_vfs::Entry;

use crate::i18n;

pub const TIME_TOLERANCE_SECS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    OnlyHere,
    Newer,
    Older,
    SizeDiffers,
    KindDiffers,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderMarks {
    left: HashMap<String, Mark>,
    right: HashMap<String, Mark>,
}

impl FolderMarks {
    pub fn left(&self, name: &str) -> Option<Mark> {
        self.left.get(name).copied()
    }

    pub fn right(&self, name: &str) -> Option<Mark> {
        self.right.get(name).copied()
    }

    pub fn is_identical(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }

    pub fn summary(&self) -> String {
        match self.difference_count() {
            0 => i18n::text("folders.compare.identical"),
            count => i18n::plural("folders.compare.differences", count),
        }
    }

    pub fn difference_count(&self) -> usize {
        let paired = self.left.keys().filter(|name| self.right.contains_key(*name)).count();
        self.left.len() + self.right.len() - paired
    }
}

pub fn needs_server_message() -> String {
    i18n::text("folders.compare.needs_server")
}

fn paired_marks(left: &Entry, right: &Entry) -> Option<(Mark, Mark)> {
    if left.is_dir != right.is_dir {
        return Some((Mark::KindDiffers, Mark::KindDiffers));
    }
    if left.is_dir {
        return None;
    }
    if let (Some(left_time), Some(right_time)) = (left.modified, right.modified) {
        if left_time > right_time.saturating_add(TIME_TOLERANCE_SECS) {
            return Some((Mark::Newer, Mark::Older));
        }
        if right_time > left_time.saturating_add(TIME_TOLERANCE_SECS) {
            return Some((Mark::Older, Mark::Newer));
        }
    }
    (left.size != right.size).then_some((Mark::SizeDiffers, Mark::SizeDiffers))
}

pub fn compare_folders(left: &[Entry], right: &[Entry]) -> FolderMarks {
    let right_by_name: HashMap<&str, &Entry> = right.iter().map(|entry| (entry.name.as_str(), entry)).collect();
    let left_names: HashMap<&str, &Entry> = left.iter().map(|entry| (entry.name.as_str(), entry)).collect();
    let mut marks = FolderMarks::default();
    for entry in left {
        match right_by_name.get(entry.name.as_str()) {
            None => {
                marks.left.insert(entry.name.clone(), Mark::OnlyHere);
            }
            Some(other) => {
                if let Some((here, there)) = paired_marks(entry, other) {
                    marks.left.insert(entry.name.clone(), here);
                    marks.right.insert(entry.name.clone(), there);
                }
            }
        }
    }
    for entry in right {
        if !left_names.contains_key(entry.name.as_str()) {
            marks.right.insert(entry.name.clone(), Mark::OnlyHere);
        }
    }
    marks
}

#[cfg(test)]
mod tests;

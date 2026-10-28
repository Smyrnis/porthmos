use crate::VfsPath;

pub const PART_SUFFIX: &str = ".part";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: VfsPath,
    pub is_dir: bool,
    pub size: u64,
    pub permissions: Option<u32>,
    pub modified: Option<u64>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub link_target: Option<String>,
    pub hidden: bool,
}

impl Entry {
    pub fn is_hidden(&self) -> bool {
        self.hidden || self.name.starts_with('.')
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    pub size: u64,
    pub modified: Option<u64>,
    pub kind: FileKind,
    pub permissions: Option<u32>,
}

impl Metadata {
    pub fn is_dir(&self) -> bool {
        self.kind == FileKind::Dir
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirItem {
    pub name: String,
    pub path: VfsPath,
    pub metadata: Metadata,
}

pub fn join_remote(parent: &str, name: &str) -> String {
    if parent.ends_with('/') { format!("{parent}{name}") } else { format!("{parent}/{name}") }
}

#[cfg(test)]
mod tests;

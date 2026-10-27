use std::{
    ffi::{OsStr, OsString},
    path::{Component, Path, PathBuf, Prefix, PrefixComponent},
};

use porthmos_vfs::VfsPath;

const ROOT: &str = r"\";
const UNC_START: &str = r"\\";

fn is_drive(segment: &OsStr) -> bool {
    segment.to_str().is_some_and(|text| {
        let mut characters = text.chars();
        matches!((characters.next(), characters.next(), characters.next()), (Some(letter), Some(':'), None) if letter.is_ascii_alphabetic())
    })
}

fn is_prefix(segment: &OsStr) -> bool {
    is_drive(segment) || segment.to_str().is_some_and(|text| text.starts_with(UNC_START))
}

fn prefix_segment(prefix: PrefixComponent<'_>) -> OsString {
    match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
            OsString::from(format!("{}:", char::from(letter.to_ascii_uppercase())))
        }
        _ => prefix.as_os_str().to_os_string(),
    }
}

pub fn to_os_path(path: &VfsPath) -> PathBuf {
    let mut segments = path.segments().peekable();
    let mut os_path = PathBuf::new();
    if path.is_absolute() {
        if let Some(prefix) = segments.next_if(|segment| is_prefix(segment)) {
            os_path.push(prefix);
        }
        os_path.push(ROOT);
    }
    os_path.extend(segments);
    os_path
}

pub fn from_os_path(path: &Path) -> VfsPath {
    let segments = path.components().filter_map(|component| match component {
        Component::Normal(name) => Some(name.to_os_string()),
        Component::ParentDir => Some(OsString::from("..")),
        Component::Prefix(prefix) => Some(prefix_segment(prefix)),
        Component::RootDir | Component::CurDir => None,
    });
    VfsPath::from_segments(path.has_root(), segments)
}

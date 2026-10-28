use porthmos_vfs::{NameRules, VfsPath};

const PARENT: &str = "..";
const UNC_START: &str = r"\\";
const NO_DRIVE: &str = "paths on Windows start with a drive, like /C:/";

fn is_drive(segment: &str) -> bool {
    let mut characters = segment.chars();
    matches!((characters.next(), characters.next(), characters.next()), (Some(letter), Some(':'), None) if letter.is_ascii_alphabetic())
}

fn is_prefix(segment: &str) -> bool {
    is_drive(segment) || segment.starts_with(UNC_START)
}

pub(crate) fn path_problem(path: &VfsPath) -> Option<String> {
    let mut segments = path.segments();
    if path.is_absolute() {
        let prefix = segments.next().and_then(|segment| segment.to_str());
        if !prefix.is_some_and(is_prefix) {
            return Some(NO_DRIVE.to_string());
        }
    }
    segments.filter_map(|segment| segment.to_str()).filter(|segment| *segment != PARENT).find_map(|segment| {
        NameRules::Windows
            .problem(segment)
            .map(|problem| format!("'{segment}' isn't a valid Windows file name ({problem})"))
    })
}

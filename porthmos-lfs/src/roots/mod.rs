use porthmos_vfs::VfsPath;
#[cfg(any(windows, test))]
use porthmos_vfs::{DirItem, Entry, FileKind, Metadata};

#[cfg(any(windows, test))]
const LETTERS: u32 = 26;

#[cfg(any(windows, test))]
pub(crate) fn drive_letters(mask: u32) -> Vec<char> {
    (0..LETTERS).filter(|bit| mask & (1 << bit) != 0).map(|bit| char::from(b'A' + bit as u8)).collect()
}

#[cfg(unix)]
pub(crate) fn lists_drives(_path: &VfsPath) -> bool {
    false
}

#[cfg(windows)]
pub(crate) fn lists_drives(path: &VfsPath) -> bool {
    path.is_root()
}

#[cfg(windows)]
pub(crate) fn present_drives() -> Vec<char> {
    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;

    drive_letters(unsafe { GetLogicalDrives() })
}

#[cfg(any(windows, test))]
fn drive_name(letter: char) -> String {
    format!("{letter}:")
}

#[cfg(any(windows, test))]
pub(crate) fn root_metadata() -> Metadata {
    Metadata { size: 0, modified: None, kind: FileKind::Dir, permissions: None }
}

#[cfg(any(windows, test))]
pub(crate) fn entries_for(letters: &[char]) -> Vec<Entry> {
    letters
        .iter()
        .map(|&letter| {
            let name = drive_name(letter);
            Entry { path: VfsPath::root().join(&name), name, is_dir: true, ..Entry::default() }
        })
        .collect()
}

#[cfg(any(windows, test))]
pub(crate) fn items_for(letters: &[char]) -> Vec<DirItem> {
    letters
        .iter()
        .map(|&letter| {
            let name = drive_name(letter);
            DirItem { path: VfsPath::root().join(&name), name, metadata: root_metadata() }
        })
        .collect()
}

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use porthmos_vfs::VfsPath;

use super::*;

#[test]
fn an_absolute_os_path_round_trips() {
    let os = Path::new("/home/user/docs");

    let path = from_os_path(os);

    assert_eq!(path, VfsPath::new("/home/user/docs"));
    assert_eq!(to_os_path(&path), PathBuf::from(os));
}

#[test]
fn the_root_round_trips() {
    assert_eq!(from_os_path(Path::new("/")), VfsPath::root());
    assert_eq!(to_os_path(&VfsPath::root()), PathBuf::from("/"));
}

#[test]
fn a_relative_os_path_round_trips() {
    let os = Path::new("a/../b");

    let path = from_os_path(os);

    assert_eq!(path, VfsPath::new("a/../b"));
    assert_eq!(to_os_path(&path), PathBuf::from(os));
}

#[test]
fn redundant_separators_and_dots_in_an_os_path_are_dropped() {
    assert_eq!(from_os_path(Path::new("/a//b/./c/")), VfsPath::new("/a/b/c"));
}

#[cfg(unix)]
#[test]
fn a_non_utf8_name_round_trips() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    let os = Path::new("/srv").join(OsStr::from_bytes(b"caf\xe9"));

    let path = from_os_path(&os);

    assert_eq!(path.file_name(), Some(OsStr::from_bytes(b"caf\xe9")));
    assert_eq!(to_os_path(&path), os);
}

#[cfg(windows)]
#[test]
fn a_drive_path_becomes_an_absolute_path_whose_first_segment_is_the_drive() {
    assert_eq!(from_os_path(Path::new(r"C:\Users\me")), VfsPath::new("/C:/Users/me"));
}

#[cfg(windows)]
#[test]
fn a_drive_root_is_the_drive_segment_alone() {
    assert_eq!(from_os_path(Path::new(r"C:\")), VfsPath::new("/C:"));
}

#[cfg(windows)]
#[test]
fn a_lower_case_drive_letter_is_upper_cased() {
    assert_eq!(from_os_path(Path::new(r"c:\temp")), VfsPath::new("/C:/temp"));
}

#[cfg(windows)]
#[test]
fn a_vfs_path_with_a_drive_goes_back_to_a_drive_path() {
    assert_eq!(to_os_path(&VfsPath::new("/C:/Users/me")), PathBuf::from(r"C:\Users\me"));
    assert_eq!(to_os_path(&VfsPath::new("/C:")), PathBuf::from(r"C:\"));
}

#[cfg(windows)]
#[test]
fn a_unc_path_round_trips() {
    let os = Path::new(r"\\server\share\dir");

    assert_eq!(to_os_path(&from_os_path(os)), PathBuf::from(os));
}

#[cfg(windows)]
#[test]
fn a_relative_windows_path_round_trips() {
    let os = Path::new(r"dir\file.txt");

    assert_eq!(from_os_path(os), VfsPath::new("dir/file.txt"));
    assert_eq!(to_os_path(&VfsPath::new("dir/file.txt")), PathBuf::from(os));
}

#[cfg(windows)]
#[test]
fn the_root_alone_has_no_drive_and_maps_to_the_backslash_root() {
    assert_eq!(to_os_path(&VfsPath::root()), PathBuf::from(r"\"));
}

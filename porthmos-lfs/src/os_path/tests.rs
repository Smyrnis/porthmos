use std::path::{Path, PathBuf};

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

use porthmos_core::VfsPath;

use super::crumbs;

fn labels(path: &VfsPath) -> Vec<String> {
    crumbs(path).into_iter().map(|crumb| crumb.label).collect()
}

fn paths(path: &VfsPath) -> Vec<String> {
    crumbs(path).into_iter().map(|crumb| crumb.path.to_string_lossy().into_owned()).collect()
}

#[test]
fn posix_paths_split_from_the_root() {
    let path = VfsPath::new("/a/b");

    assert_eq!(labels(&path), ["/", "a", "b"]);
    assert_eq!(paths(&path), ["/", "/a", "/a/b"]);
}

#[test]
fn the_root_is_one_crumb() {
    let root = VfsPath::root();

    assert_eq!(labels(&root), ["/"]);
    assert_eq!(paths(&root), ["/"]);
}

#[test]
fn windows_drive_paths_start_at_the_drive() {
    let path = VfsPath::from_segments(true, ["C:", "Users", "me"]);

    assert_eq!(labels(&path), ["C:", "Users", "me"]);
    assert_eq!(paths(&path), ["/C:", "/C:/Users", "/C:/Users/me"]);
}

#[test]
fn a_bare_drive_is_one_crumb() {
    let path = VfsPath::from_segments(true, ["D:"]);

    assert_eq!(labels(&path), ["D:"]);
}

#[test]
fn a_relative_path_is_one_dot_crumb() {
    assert_eq!(labels(&VfsPath::new("a/b")), ["."]);
    assert_eq!(labels(&VfsPath::new("")), ["."]);
}

#[test]
fn a_name_that_only_looks_like_a_drive_is_still_a_folder() {
    let path = VfsPath::new("/data/C:x");

    assert_eq!(labels(&path), ["/", "data", "C:x"]);
}

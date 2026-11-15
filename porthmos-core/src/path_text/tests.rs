use porthmos_vfs::VfsPath;

use super::*;
use crate::Location;

fn session(text: &str) -> VfsPath {
    resolve_path_text(Location::Session(1), text, &VfsPath::new("/srv/a/b"), &VfsPath::new("/home/me"))
}

#[test]
fn tilde_alone_is_home() {
    assert_eq!(session("~"), VfsPath::new("/home/me"));
}

#[test]
fn tilde_slash_rest_is_under_home() {
    assert_eq!(session("~/x/y"), VfsPath::new("/home/me/x/y"));
}

#[test]
fn tilde_followed_by_a_name_is_relative_not_home() {
    assert_eq!(session("~x"), VfsPath::new("/srv/a/b/~x"));
}

#[test]
fn absolute_text_is_kept() {
    assert_eq!(session("/etc"), VfsPath::new("/etc"));
}

#[test]
fn relative_text_is_joined_onto_from() {
    assert_eq!(session("c/d"), VfsPath::new("/srv/a/b/c/d"));
}

#[test]
fn dot_and_dot_dot_are_resolved() {
    assert_eq!(session("../c"), VfsPath::new("/srv/a/c"));
    assert_eq!(session("./c/../d"), VfsPath::new("/srv/a/b/d"));
    assert_eq!(session("/a/b/../../c"), VfsPath::new("/c"));
}

#[test]
fn dot_dot_cannot_go_above_the_root() {
    assert_eq!(session("../../../../x"), VfsPath::new("/x"));
    assert_eq!(session("/.."), VfsPath::root());
}

#[test]
fn doubled_and_trailing_slashes_are_ignored() {
    assert_eq!(session("/a//b/"), VfsPath::new("/a/b"));
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    assert_eq!(session("  /a/b \t"), VfsPath::new("/a/b"));
    assert_eq!(session("  ~  "), VfsPath::new("/home/me"));
}

#[test]
fn tilde_dot_dot_goes_above_home() {
    assert_eq!(session("~/.."), VfsPath::new("/home"));
}

#[cfg(unix)]
#[test]
fn a_local_os_absolute_path_is_converted() {
    let resolved = resolve_path_text(Location::Local, "/tmp/x", &VfsPath::new("/srv/a/b"), &VfsPath::new("/home/me"));

    assert_eq!(resolved, porthmos_lfs::from_os_path(std::path::Path::new("/tmp/x")));
}

#[test]
fn a_session_path_keeps_a_backslash_as_a_name_character() {
    assert_eq!(session("a\\b"), VfsPath::new("/srv/a/b/a\\b"));
}

#[test]
fn only_a_bare_tilde_or_tilde_slash_needs_the_home_folder() {
    assert!(needs_home("~"));
    assert!(needs_home("  ~/docs"));
    assert!(!needs_home("~backup"));
    assert!(!needs_home("docs/~"));
    assert!(!needs_home(""));
}

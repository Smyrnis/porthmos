use porthmos_core::VfsPath;
use porthmos_core::config::bookmarks::Bookmark;

use crate::sessions::Sessions;

use super::{BookmarksState, Goto, default_label, goto, row_text};

fn local(label: &str, path: &str) -> Bookmark {
    Bookmark { label: label.to_string(), path: VfsPath::new(path), host: None }
}

fn remote(label: &str, path: &str, host: &str) -> Bookmark {
    Bookmark { label: label.to_string(), path: VfsPath::new(path), host: Some(host.to_string()) }
}

#[test]
fn default_label_is_the_last_component() {
    assert_eq!(default_label(&VfsPath::new("/home/me/projects")), "projects");
}

#[test]
fn default_label_for_a_root_is_bookmark() {
    assert_eq!(default_label(&VfsPath::new("/")), "bookmark");
    assert_eq!(default_label(&VfsPath::default()), "bookmark");
}

#[test]
fn row_text_shows_the_host_for_remote_bookmarks() {
    assert_eq!(row_text(&local("Home", "/home/me")), ("Home".to_string(), "/home/me".to_string()));
    assert_eq!(row_text(&remote("Web", "/var/www", "alpha")), ("Web".to_string(), "/var/www · alpha".to_string()));
}

#[test]
fn goto_a_local_bookmark() {
    let sessions = Sessions::default();

    assert_eq!(goto(&local("Home", "/home/me"), &sessions), Goto::Local(VfsPath::new("/home/me")));
}

#[test]
fn goto_a_remote_bookmark_with_an_open_session() {
    let mut sessions = Sessions::default();
    sessions.insert(7, "alpha".to_string());

    assert_eq!(
        goto(&remote("Web", "/var/www", "alpha"), &sessions),
        Goto::Session { id: 7, path: VfsPath::new("/var/www") }
    );
}

#[test]
fn goto_a_remote_bookmark_without_a_session_asks_to_connect() {
    let sessions = Sessions::default();

    assert_eq!(goto(&remote("Web", "/var/www", "alpha"), &sessions), Goto::NotConnected("alpha".to_string()));
}

#[test]
fn a_new_list_selects_the_first_bookmark() {
    let state = BookmarksState::new(vec![local("a", "/a"), local("b", "/b")]);

    assert_eq!(state.selected().map(|(index, bookmark)| (index, bookmark.label.as_str())), Some((0, "a")));
}

#[test]
fn an_empty_list_selects_nothing() {
    let mut state = BookmarksState::new(Vec::new());

    state.move_selection(true);

    assert!(state.selected().is_none());
}

#[test]
fn move_selection_clamps_at_both_ends() {
    let mut state = BookmarksState::new(vec![local("a", "/a"), local("b", "/b")]);

    state.move_selection(false);
    assert_eq!(state.selected().unwrap().0, 0);
    state.move_selection(true);
    state.move_selection(true);
    assert_eq!(state.selected().unwrap().0, 1);
}

#[test]
fn replace_keeps_the_selection_clamped() {
    let mut state = BookmarksState::new(vec![local("a", "/a"), local("b", "/b"), local("c", "/c")]);
    state.select(2);

    state.replace(vec![local("a", "/a"), local("b", "/b")]);

    assert_eq!(state.selected().unwrap().0, 1);
    state.replace(Vec::new());
    assert!(state.selected().is_none());
}

#[test]
fn select_ignores_a_row_past_the_end() {
    let mut state = BookmarksState::new(vec![local("a", "/a")]);

    state.select(4);

    assert_eq!(state.selected().unwrap().0, 0);
}

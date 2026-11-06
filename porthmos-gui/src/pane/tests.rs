use porthmos_core::{Entry, VfsPath};

use super::Pane;

fn entry(folder: &VfsPath, name: &str) -> Entry {
    Entry { name: name.to_string(), path: folder.join(name), ..Entry::default() }
}

#[test]
fn a_listing_of_the_folder_the_pane_shows_is_accepted() {
    let folder = VfsPath::new("/a");
    let mut pane = Pane::new(folder.clone());

    assert!(pane.accept(folder.clone(), vec![entry(&folder, "x")]));

    assert_eq!(pane.panel.rows().len(), 2);
}

#[test]
fn a_listing_of_an_unrequested_folder_is_dropped() {
    let mut pane = Pane::new(VfsPath::new("/a"));

    assert!(!pane.accept(VfsPath::new("/b"), Vec::new()));

    assert_eq!(pane.panel.path(), &VfsPath::new("/a"));
}

#[test]
fn a_requested_folder_is_accepted_once_and_then_becomes_current() {
    let mut pane = Pane::new(VfsPath::new("/a"));
    pane.expect(Some(VfsPath::new("/b")));

    assert!(pane.accept(VfsPath::new("/b"), Vec::new()));
    assert_eq!(pane.panel.path(), &VfsPath::new("/b"));

    assert!(!pane.accept(VfsPath::new("/a"), Vec::new()));
    assert!(pane.accept(VfsPath::new("/b"), Vec::new()));
}

#[test]
fn a_late_listing_of_a_folder_left_behind_is_dropped_while_another_is_requested() {
    let mut pane = Pane::new(VfsPath::new("/a"));
    pane.expect(Some(VfsPath::new("/b")));
    pane.expect(Some(VfsPath::new("/c")));

    assert!(!pane.accept(VfsPath::new("/b"), Vec::new()));
    assert!(pane.accept(VfsPath::new("/c"), Vec::new()));
    assert_eq!(pane.panel.path(), &VfsPath::new("/c"));
}

#[test]
fn refreshing_the_current_folder_is_accepted_while_a_move_is_pending() {
    let mut pane = Pane::new(VfsPath::new("/a"));
    pane.expect(Some(VfsPath::new("/b")));

    assert!(pane.accept(VfsPath::new("/a"), Vec::new()));
    assert!(pane.accept(VfsPath::new("/b"), Vec::new()));
}

#[test]
fn a_pane_waiting_for_its_first_listing_accepts_any_folder_once() {
    let mut pane = Pane::awaiting_first_listing();

    assert!(pane.accept(VfsPath::new("/home/me"), Vec::new()));
    assert_eq!(pane.panel.path(), &VfsPath::new("/home/me"));

    assert!(!pane.accept(VfsPath::new("/elsewhere"), Vec::new()));
}

#[test]
fn asking_for_the_home_folder_accepts_whatever_core_answers() {
    let mut pane = Pane::new(VfsPath::new("/a"));
    pane.expect(None);

    assert!(pane.accept(VfsPath::new("/home/me"), Vec::new()));
}

#[test]
fn the_remembered_scroll_resets_when_another_folder_is_shown() {
    let mut pane = Pane::new(VfsPath::new("/a"));
    pane.scroll = 480.0;

    pane.accept(VfsPath::new("/a"), Vec::new());
    assert_eq!(pane.scroll, 480.0);

    pane.expect(Some(VfsPath::new("/b")));
    pane.accept(VfsPath::new("/b"), Vec::new());
    assert_eq!(pane.scroll, 0.0);
}

#[test]
fn a_pane_is_listed_after_its_first_listing() {
    let mut pane = Pane::awaiting_first_listing();
    assert!(!pane.is_listed());

    assert!(pane.accept(VfsPath::new("/a"), Vec::new()));
    assert!(pane.is_listed());
}

#[test]
fn a_dropped_listing_does_not_make_a_pane_listed() {
    let mut pane = Pane::new(VfsPath::new("/a"));

    assert!(!pane.accept(VfsPath::new("/b"), Vec::new()));

    assert!(!pane.is_listed());
}

#[test]
fn a_reveal_puts_the_cursor_on_the_named_row_of_the_folder_shown() {
    let folder = VfsPath::new("/a");
    let mut pane = Pane::new(folder.clone());
    pane.accept(folder.clone(), vec![entry(&folder, "x"), entry(&folder, "y")]);

    pane.reveal(&folder, "y");

    assert_eq!(pane.panel.cursor_entry().map(|entry| entry.name), Some("y".to_string()));
    assert!(pane.take_reveal_request());
}

#[test]
fn a_reveal_for_another_folder_changes_nothing() {
    let folder = VfsPath::new("/a");
    let mut pane = Pane::new(folder.clone());
    pane.accept(folder.clone(), vec![entry(&folder, "x"), entry(&folder, "y")]);
    let before = pane.panel.cursor();

    pane.reveal(&VfsPath::new("/b"), "y");

    assert_eq!(pane.panel.cursor(), before);
    assert!(!pane.take_reveal_request());
}

#[test]
fn a_reveal_for_a_name_that_is_not_listed_asks_for_no_scrolling() {
    let folder = VfsPath::new("/a");
    let mut pane = Pane::new(folder.clone());
    pane.accept(folder.clone(), vec![entry(&folder, "x")]);

    pane.reveal(&folder, "gone");

    assert!(!pane.take_reveal_request());
}

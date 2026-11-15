use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::{Entry, VfsPath};

use crate::{
    app::testing::{TestApp, entry, find_text, render_app, test_app},
    input::Action,
};

fn dated(dir: &str, name: &str, modified: u64) -> Entry {
    Entry { modified: Some(modified), ..entry(&VfsPath::from(dir), name, false) }
}

fn both_sides() -> TestApp {
    let mut test = test_app(&VfsPath::new("/d"));
    test.connect(3, "prod");
    test.list_local(vec![dated("/d", "same.txt", 100), dated("/d", "mine.txt", 100), dated("/d", "news.txt", 5000)]);
    test.list_remote(
        3,
        "/srv",
        vec![dated("/srv", "same.txt", 100), dated("/srv", "theirs.txt", 100), dated("/srv", "news.txt", 10)],
    );
    test.sent();
    test
}

fn press(test: &mut TestApp, key: KeyEvent) {
    test.app.apply_key(key);
}

fn equals_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE)
}

#[test]
fn the_action_has_a_default_key() {
    let mut test = both_sides();

    press(&mut test, equals_key());

    assert!(test.app.compare_folders);
}

#[test]
fn turning_it_on_reports_how_many_entries_differ() {
    let mut test = both_sides();

    test.app.apply_action(Action::CompareFolders);

    assert!(test.app.compare_folders);
    assert_eq!(test.notification().as_deref(), Some("Comparing folders: 3 differences"));
}

#[test]
fn identical_folders_are_reported_as_identical() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.connect(3, "prod");
    test.list_local(vec![dated("/d", "a", 1)]);
    test.list_remote(3, "/srv", vec![dated("/srv", "a", 1)]);

    test.app.apply_action(Action::CompareFolders);

    assert_eq!(test.notification().as_deref(), Some("Comparing folders: they are identical"));
}

#[test]
fn a_single_difference_is_not_pluralised() {
    let mut test = test_app(&VfsPath::new("/d"));
    test.connect(3, "prod");
    test.list_local(vec![dated("/d", "a", 1)]);
    test.list_remote(3, "/srv", Vec::new());

    test.app.apply_action(Action::CompareFolders);

    assert_eq!(test.notification().as_deref(), Some("Comparing folders: 1 difference"));
}

#[test]
fn without_a_connected_server_it_stays_off_and_says_why() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_action(Action::CompareFolders);

    assert!(!test.app.compare_folders);
    assert_eq!(test.notification().as_deref(), Some("Connect to a server to compare folders"));
}

#[test]
fn the_second_press_turns_it_off() {
    let mut test = both_sides();
    test.app.apply_action(Action::CompareFolders);

    test.app.apply_action(Action::CompareFolders);

    assert!(!test.app.compare_folders);
}

const SCREEN_WIDTH: u16 = 120;

fn mark_beside(screen: &ratatui::Terminal<ratatui::backend::TestBackend>, name: &str, right_panel: bool) -> String {
    let buffer = screen.backend().buffer();
    let (from, to) = if right_panel { (SCREEN_WIDTH / 2, SCREEN_WIDTH) } else { (0, SCREEN_WIDTH / 2) };
    for y in 0..buffer.area.height {
        let row: Vec<String> = (from..to).map(|x| buffer[(x, y)].symbol().to_string()).collect();
        let text = row.concat();
        if let Some(offset) = text.find(name) {
            let column = text[..offset].chars().count();
            if column > 0 {
                return row[column - 1].clone();
            }
        }
    }
    panic!("{name} is not drawn in that panel");
}

#[test]
fn the_local_panel_shows_its_marks_when_it_is_on() {
    let mut test = both_sides();
    test.app.apply_action(Action::CompareFolders);

    let screen = render_app(&test.app, SCREEN_WIDTH, 20);

    assert_eq!(mark_beside(&screen, "mine.txt", false), "\u{2022}");
    assert_eq!(mark_beside(&screen, "news.txt", false), "\u{2191}");
    assert_eq!(mark_beside(&screen, "same.txt", false), " ");
}

#[test]
fn the_remote_panel_shows_the_other_sides_view() {
    let mut test = both_sides();
    test.app.apply_action(Action::CompareFolders);

    let screen = render_app(&test.app, SCREEN_WIDTH, 20);

    assert_eq!(mark_beside(&screen, "theirs.txt", true), "\u{2022}");
    assert_eq!(mark_beside(&screen, "news.txt", true), "\u{2193}");
    assert_eq!(mark_beside(&screen, "same.txt", true), " ");
}

#[test]
fn nothing_is_marked_while_it_is_off() {
    let test = both_sides();

    let screen = render_app(&test.app, SCREEN_WIDTH, 20);

    assert_eq!(mark_beside(&screen, "mine.txt", false), " ");
    assert_eq!(mark_beside(&screen, "theirs.txt", true), " ");
}

#[test]
fn the_palette_offers_it() {
    let mut test = both_sides();

    test.app.apply_action(Action::OpenPalette);
    test.type_text("compare");

    let screen = render_app(&test.app, 120, 30);
    assert!(find_text(&screen, "Compare folders").is_some());
}

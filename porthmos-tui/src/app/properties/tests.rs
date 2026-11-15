use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::{
    Command, Event, Location, VfsPath,
    properties::{Digest, Measured},
};

use crate::{
    app::{
        Screen,
        testing::{TestApp, entry, find_text, has_unprintable_cell, render_app, test_app},
    },
    widgets::dialog::Dialog,
};

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn app_with(names: &[(&str, bool)]) -> TestApp {
    let mut test = test_app(&VfsPath::new("/d"));
    test.list_local(names.iter().map(|(name, is_dir)| entry(&VfsPath::new("/d"), name, *is_dir)).collect());
    test
}

fn on_entry(test: &mut TestApp, index: usize) {
    test.app.local.cursor = index + 1;
}

fn measured(bytes: u64, files: u64) -> Measured {
    Measured { bytes, files, ..Measured::default() }
}

fn dialog_is_open(test: &TestApp) -> bool {
    matches!(test.app.dialog, Some(Dialog::Properties(_)))
}

#[test]
fn the_key_i_opens_the_dialog_and_measures_the_entry_under_the_cursor() {
    let mut test = app_with(&[("a.iso", false)]);
    on_entry(&mut test, 0);

    test.type_text("i");

    assert!(dialog_is_open(&test));
    assert_eq!(
        test.sent(),
        vec![Command::Measure { request_id: 1, location: Location::Local, paths: vec![VfsPath::new("/d/a.iso")] }]
    );
}

#[test]
fn a_selection_is_measured_as_a_whole_and_each_open_gets_a_new_request_id() {
    let mut test = app_with(&[("a.iso", false), ("b", true)]);
    for index in 0..2 {
        on_entry(&mut test, index);
        test.app.local.toggle_selection();
    }

    test.app.apply_action(crate::input::Action::Properties);
    let first = test.sent();
    test.app.apply_key(key(KeyCode::Esc));
    test.app.apply_action(crate::input::Action::Properties);
    let second = test.sent();

    let Some(Command::Measure { request_id: 1, paths, .. }) = first.first() else { panic!("{first:?}") };
    let mut paths = paths.clone();
    paths.sort();
    assert_eq!(paths, vec![VfsPath::new("/d/a.iso"), VfsPath::new("/d/b")]);
    assert!(matches!(second.last(), Some(Command::Measure { request_id: 2, .. })), "{second:?}");
}

#[test]
fn nothing_opens_on_the_parent_row_alone() {
    let mut test = app_with(&[("a.iso", false)]);
    test.app.local.cursor = 0;

    test.type_text("i");

    assert!(!dialog_is_open(&test));
    assert!(test.sent().is_empty());
}

#[test]
fn nothing_opens_off_the_files_screen() {
    let mut test = app_with(&[("a.iso", false)]);
    on_entry(&mut test, 0);
    test.app.screen = Screen::Connections;

    test.app.apply_action(crate::input::Action::Properties);

    assert!(!dialog_is_open(&test));
    assert!(test.sent().is_empty());
}

#[test]
fn s_asks_for_checksums_of_the_files_only_with_the_same_request_id() {
    let mut test = app_with(&[("a.iso", false), ("b", true)]);
    for index in 0..2 {
        on_entry(&mut test, index);
        test.app.local.toggle_selection();
    }
    test.app.apply_action(crate::input::Action::Properties);
    test.sent();

    test.app.apply_key(key(KeyCode::Char('s')));

    assert_eq!(
        test.sent(),
        vec![Command::Checksum { request_id: 1, location: Location::Local, paths: vec![VfsPath::new("/d/a.iso")] }]
    );
}

#[test]
fn a_selection_of_folders_only_has_nothing_to_checksum() {
    let mut test = app_with(&[("b", true)]);
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);
    test.sent();

    test.app.apply_key(key(KeyCode::Char('s')));

    assert!(test.sent().is_empty());
}

#[test]
fn measure_events_update_the_open_dialog() {
    let mut test = app_with(&[("a.iso", false)]);
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);

    test.app.apply_core_event(Event::MeasureProgress { request_id: 1, measured: measured(0, 812) });
    assert!(find_text(&render_app(&test.app, 100, 30), "812 files so far").is_some());
    test.app.apply_core_event(Event::Measured { request_id: 1, result: Ok(measured(4_521_983_744, 1_204)) });

    assert!(find_text(&render_app(&test.app, 100, 30), "4.2 GB (4,521,983,744 bytes)").is_some());
}

#[test]
fn checksum_events_update_the_open_dialog() {
    let mut test = app_with(&[("a.iso", false)]);
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);
    test.app.apply_core_event(Event::Measured { request_id: 1, result: Ok(measured(3, 1)) });
    test.app.apply_key(key(KeyCode::Char('s')));

    test.app.apply_core_event(Event::ChecksumProgress {
        request_id: 1,
        path: VfsPath::new("/d/a.iso"),
        bytes_done: 1_024,
        bytes_total: 2_048,
    });
    assert!(find_text(&render_app(&test.app, 100, 30), "1.0 KB of 2.0 KB").is_some());
    test.app.apply_core_event(Event::Checksummed {
        request_id: 1,
        path: VfsPath::new("/d/a.iso"),
        result: Ok(Digest { hex: ABC.to_string(), bytes: 3 }),
    });

    assert!(find_text(&render_app(&test.app, 100, 30), ABC).is_some());
}

#[test]
fn events_for_another_request_or_with_no_dialog_are_ignored() {
    let mut test = app_with(&[("a.iso", false)]);
    test.app.apply_core_event(Event::Measured { request_id: 9, result: Ok(measured(1, 1)) });
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);

    test.app.apply_core_event(Event::Measured { request_id: 9, result: Ok(measured(7_000, 1)) });
    test.app.apply_core_event(Event::Checksummed {
        request_id: 9,
        path: VfsPath::new("/d/a.iso"),
        result: Ok(Digest { hex: ABC.to_string(), bytes: 3 }),
    });

    let terminal = render_app(&test.app, 100, 30);
    assert!(find_text(&terminal, "6.8 KB").is_none());
    assert!(find_text(&terminal, ABC).is_none());
}

#[test]
fn closing_sends_cancel_with_the_request_id_and_forgets_the_dialog() {
    let mut test = app_with(&[("a.iso", false)]);
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);
    test.app.apply_core_event(Event::Measured { request_id: 1, result: Ok(measured(1, 1)) });
    test.sent();

    test.app.apply_key(key(KeyCode::Esc));

    assert!(!dialog_is_open(&test));
    assert_eq!(test.sent(), vec![Command::CancelProperties { request_id: 1 }]);
    test.app.apply_core_event(Event::Measured { request_id: 1, result: Ok(measured(5, 1)) });
}

#[test]
fn esc_while_running_cancels_but_keeps_the_dialog_and_the_next_esc_closes() {
    let mut test = app_with(&[("a.iso", false)]);
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);
    test.sent();

    test.app.apply_key(key(KeyCode::Esc));
    assert!(dialog_is_open(&test));
    assert_eq!(test.sent(), vec![Command::CancelProperties { request_id: 1 }]);

    test.app.apply_key(key(KeyCode::Esc));
    assert!(!dialog_is_open(&test));
    assert_eq!(test.sent(), vec![Command::CancelProperties { request_id: 1 }]);
}

#[test]
fn a_hostile_file_name_is_printable_in_the_dialog() {
    let mut test = app_with(&[("bad\u{1b}[31mname\n\u{202e}gpj", false)]);
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);
    test.app.apply_core_event(Event::Measured { request_id: 1, result: Ok(measured(3, 1)) });

    assert!(!has_unprintable_cell(&render_app(&test.app, 100, 30)));
}

#[test]
fn the_palette_offers_the_action_with_its_key_on_the_files_screen() {
    let test = app_with(&[("a.iso", false)]);

    let entries = test.app.palette_entries();

    let entry = entries
        .iter()
        .find(|entry| matches!(entry.target, crate::palette::Target::Action(crate::input::Action::Properties)))
        .expect("a palette entry");
    assert_eq!(entry.hint.as_deref(), Some("i"));
}

#[test]
fn a_core_question_replacing_the_dialog_cancels_the_running_jobs() {
    let mut test = app_with(&[("a.iso", false)]);
    on_entry(&mut test, 0);
    test.app.apply_action(crate::input::Action::Properties);
    test.sent();

    test.app.apply_core_event(Event::Question {
        request_id: 40,
        question: porthmos_core::Question::Password { username: "u".into(), name: "web".into(), secret: "Password" },
    });

    assert!(!dialog_is_open(&test));
    let sent = test.sent();
    assert!(sent.contains(&Command::CancelProperties { request_id: 1 }), "{sent:?}");
}

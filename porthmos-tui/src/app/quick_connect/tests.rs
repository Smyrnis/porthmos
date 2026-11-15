use chrono::{TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use porthmos_core::VfsPath;
use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};
use porthmos_core::{Command, Event};

use super::*;
use crate::app::Screen;
use crate::app::testing::{TestApp, test_app};

fn app() -> TestApp {
    test_app(&VfsPath::new("/d"))
}

fn entry(name: &str, source: ConnectionSource) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: format!("{name}.example.com"),
        port: 22,
        username: "deploy".to_string(),
        password: None,
        options: Default::default(),
        source,
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
    }
}

fn feed(test: &mut TestApp, names: &[&str]) {
    let entries = names.iter().map(|name| entry(name, ConnectionSource::Profile)).collect();
    test.app.apply_core_event(Event::Profiles(entries));
    test.sent();
}

fn used(test: &mut TestApp, list: &[(&str, u32)]) {
    let times = list
        .iter()
        .map(|(name, day)| (name.to_string(), Utc.with_ymd_and_hms(2026, 10, *day, 12, 0, 0).unwrap()))
        .collect();
    test.app.apply_core_event(Event::LastUsed(times));
}

fn press(test: &mut TestApp, code: KeyCode) {
    test.app.apply_key(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    });
}

#[test]
fn without_history_connections_come_in_the_order_core_lists_them() {
    let mut test = app();
    feed(&mut test, &["nas", "prod", "staging"]);

    assert_eq!(test.app.quick_connections(), vec!["nas", "prod", "staging"]);
}

#[test]
fn recently_used_connections_come_first_newest_first_then_the_rest_in_listed_order() {
    let mut test = app();
    feed(&mut test, &["a", "b", "c", "d"]);
    used(&mut test, &[("c", 3), ("b", 9)]);

    assert_eq!(test.app.quick_connections(), vec!["b", "c", "a", "d"]);
}

#[test]
fn rows_that_cannot_be_connected_are_not_offered() {
    let mut test = app();
    test.app.apply_core_event(Event::Profiles(vec![
        entry("real", ConnectionSource::Profile),
        entry("box", ConnectionSource::SshConfig),
        entry("gone", ConnectionSource::MissingSshHost),
        entry("hidden", ConnectionSource::ShadowedSshHost),
    ]));

    assert_eq!(test.app.quick_connections(), vec!["real", "box"]);
}

#[test]
fn at_most_five_are_offered_and_the_rest_are_counted() {
    let mut test = app();
    feed(&mut test, &["a", "b", "c", "d", "e", "f", "g"]);

    assert_eq!(test.app.quick_connections(), vec!["a", "b", "c", "d", "e"]);
    assert_eq!(test.app.quick_connections_hidden(), 2);
}

#[test]
fn a_digit_connects_the_connection_with_that_number() {
    let mut test = app();
    feed(&mut test, &["nas", "prod"]);

    press(&mut test, KeyCode::Char('2'));

    assert_eq!(test.sent(), vec![Command::Connect { profile: "prod".to_string() }]);
}

#[test]
fn the_numbers_follow_the_recently_used_order() {
    let mut test = app();
    feed(&mut test, &["nas", "prod"]);
    used(&mut test, &[("prod", 5)]);

    press(&mut test, KeyCode::Char('1'));

    assert_eq!(test.sent(), vec![Command::Connect { profile: "prod".to_string() }]);
}

#[test]
fn a_digit_with_no_such_connection_does_nothing() {
    let mut test = app();
    feed(&mut test, &["nas"]);

    for digit in ['0', '2', '9'] {
        press(&mut test, KeyCode::Char(digit));
    }

    assert!(test.sent().is_empty());
}

#[test]
fn a_digit_does_nothing_while_a_connection_is_being_made() {
    let mut test = app();
    feed(&mut test, &["nas"]);
    test.app.connection_status = ConnectionStatus::Connecting("nas".to_string());

    press(&mut test, KeyCode::Char('1'));

    assert!(test.sent().is_empty());
}

#[test]
fn a_digit_does_nothing_on_other_screens() {
    let mut test = app();
    feed(&mut test, &["nas"]);
    test.app.screen = Screen::Connections;

    press(&mut test, KeyCode::Char('1'));

    assert!(!test.sent().iter().any(|command| matches!(command, Command::Connect { .. })));
}

#[test]
fn a_digit_with_a_modifier_is_left_alone() {
    let mut test = app();
    feed(&mut test, &["nas"]);

    test.app.apply_key(KeyEvent {
        code: KeyCode::Char('1'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    });

    assert!(test.sent().is_empty());
}

#[test]
fn the_placeholder_lists_numbered_connections_and_points_to_the_screen_for_the_rest() {
    let mut test = app();
    feed(&mut test, &["a", "b", "c", "d", "e", "f"]);

    let lines = test.app.remote_placeholder_lines();

    assert_eq!(lines[0], "not connected");
    assert_eq!(&lines[1..6], ["1  a", "2  b", "3  c", "4  d", "5  e"]);
    assert_eq!(lines[6], "press F9 for 1 more");
}

#[test]
fn the_placeholder_points_to_the_screen_for_all_when_everything_fits() {
    let mut test = app();
    feed(&mut test, &["a"]);

    let lines = test.app.remote_placeholder_lines();

    assert_eq!(lines, vec!["not connected", "1  a", "press F9 for all connections"]);
}

#[test]
fn the_placeholder_without_saved_connections_says_how_to_add_one() {
    let test = app();

    let lines = test.app.remote_placeholder_lines();

    assert_eq!(lines, vec!["not connected", "press F9 to add a connection"]);
}

#[test]
fn names_are_made_printable_in_the_placeholder() {
    let mut test = app();
    feed(&mut test, &["bad\u{1b}name"]);

    let lines = test.app.remote_placeholder_lines();

    assert!(!lines.iter().any(|line| line.contains('\u{1b}')), "{lines:?}");
}

#[test]
fn the_reopen_offer_still_comes_before_the_list() {
    let mut test = app();
    feed(&mut test, &["a"]);
    test.app.apply_core_event(Event::ReopenOffer { names: vec!["a".to_string()] });

    let lines = test.app.remote_placeholder_lines();

    let offer = lines.iter().position(|line| line.starts_with("last time:")).unwrap();
    let list = lines.iter().position(|line| line == "1  a").unwrap();
    assert!(offer < list, "{lines:?}");
}

#[test]
fn while_connecting_the_placeholder_only_says_so() {
    let mut test = app();
    feed(&mut test, &["a"]);
    test.app.connection_status = ConnectionStatus::Connecting("a".to_string());

    assert_eq!(test.app.remote_placeholder_lines(), vec!["connecting to a\u{2026}"]);
}

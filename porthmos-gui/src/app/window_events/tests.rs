use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use iced::{Point, Size, window};

use super::super::tests::{desktop_files, drag_point, drop_ready};
use super::super::*;
use crate::shortcuts::KeyPress;
use crate::testing::Harness;
use crate::windows::WindowKind;

fn escape() -> KeyPress {
    let key = Key::Named(Named::Escape);
    KeyPress { key: key.clone(), modified_key: key, modifiers: Modifiers::empty(), text: None }
}

fn open_path_field(harness: &mut Harness) {
    harness.press(Message::PathEditStarted(PanelSide::Local));
    assert!(harness.app.path_edit().is_some());
}

#[test]
fn the_harness_app_knows_its_main_window() {
    let harness = Harness::new();

    assert_eq!(harness.app.main_window(), harness.main_id());
    assert!(harness.app.is_main(harness.main_id()));
    assert!(!harness.app.is_main(window::Id::unique()));
}

#[test]
fn a_key_from_the_main_window_runs_its_handling() {
    let mut harness = Harness::new();
    open_path_field(&mut harness);
    let main = harness.main_id();

    harness.press(Message::WindowKey(main, escape()));

    assert!(harness.app.path_edit().is_none());
}

#[test]
fn a_key_from_an_unknown_window_does_nothing() {
    let mut harness = Harness::new();
    open_path_field(&mut harness);

    harness.press(Message::WindowKey(window::Id::unique(), escape()));

    assert!(harness.app.path_edit().is_some());
}

#[test]
fn a_resize_of_the_main_window_updates_the_window_size() {
    let mut harness = Harness::new();
    let main = harness.main_id();

    harness.press(Message::WindowSized(main, Size::new(900.0, 500.0)));

    assert_eq!(harness.app.window_size(), Size::new(900.0, 500.0));
}

#[test]
fn a_resize_of_an_unknown_window_is_ignored() {
    let mut harness = Harness::new();
    let before = harness.app.window_size();

    harness.press(Message::WindowSized(window::Id::unique(), Size::new(10.0, 10.0)));

    assert_eq!(harness.app.window_size(), before);
}

#[test]
fn a_close_request_of_the_main_window_starts_the_quit_flow() {
    let mut harness = Harness::new();
    let main = harness.main_id();

    harness.press(Message::WindowCloseRequested(main));

    assert!(harness.app.is_closing());
}

#[test]
fn a_close_request_of_an_unknown_window_is_ignored() {
    let mut harness = Harness::new();

    harness.press(Message::WindowCloseRequested(window::Id::unique()));

    assert!(!harness.app.is_closing());
}

#[test]
fn pointer_events_from_the_main_window_are_applied() {
    let mut harness = Harness::new();
    let main = harness.main_id();

    harness.press(Message::FromWindow(main, Box::new(Message::PointerMoved(Point::new(5.0, 6.0)))));

    assert_eq!(harness.app.pointer_position(), Some(Point::new(5.0, 6.0)));
}

#[test]
fn pointer_events_from_another_window_are_ignored() {
    let mut harness = Harness::new();

    harness.press(Message::FromWindow(window::Id::unique(), Box::new(Message::PointerMoved(Point::new(5.0, 6.0)))));

    assert_eq!(harness.app.pointer_position(), None);
}

#[test]
fn the_title_follows_the_window_kind() {
    let harness = Harness::new();

    assert_eq!(harness.app.title_of(harness.main_id()), "Porthmos");
    assert_eq!(WindowKind::Settings.title(), "Porthmos Settings");
    assert_eq!(WindowKind::Connections.title(), "Porthmos Connections");
    assert_eq!(harness.app.title_of(window::Id::unique()), "Porthmos");
}

#[test]
fn a_file_dropped_on_a_secondary_window_does_nothing() {
    let mut harness = Harness::new();
    drop_ready(&mut harness, &[]);
    harness.press(Message::ActionRequested(Action::Settings));
    let settings = harness.settings_id().unwrap();
    let (_keep, paths) = desktop_files(&["a.txt"]);
    let at = drag_point(&harness, PanelSide::Remote, 10);
    let dropped = crate::native::DesktopEvent::Dropped { paths, at: Some(at) };

    harness.sent();
    harness.press(Message::FromWindow(settings, Box::new(Message::Desktop(dropped))));
    harness.press(Message::DesktopDropFlush);

    assert_eq!(harness.sent(), Vec::new());
}

#[test]
fn a_close_request_for_an_unregistered_window_asks_iced_to_close_it() {
    let mut harness = Harness::new();

    let units = harness.app.update(Message::WindowCloseRequested(window::Id::unique())).units();

    assert_eq!(units, 1);
}

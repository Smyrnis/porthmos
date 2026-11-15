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

    harness.press(Message::FromWindow(main, Box::new(Message::CursorMoved(Point::new(5.0, 6.0)))));

    assert_eq!(harness.app.pointer_position(), Some(Point::new(5.0, 6.0)));
}

#[test]
fn pointer_events_from_another_window_are_ignored() {
    let mut harness = Harness::new();

    harness.press(Message::FromWindow(window::Id::unique(), Box::new(Message::CursorMoved(Point::new(5.0, 6.0)))));

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

#[test]
fn a_move_of_the_main_window_is_remembered() {
    let mut harness = Harness::new();
    let main = harness.main_id();

    harness.press(Message::WindowMoveSettled(main, Point::new(150.0, 90.0), false));

    let settings = harness.app.windows.memory().applied_to(WindowKind::Main, crate::main_window::settings());
    assert!(matches!(settings.position, window::Position::Specific(p) if p == Point::new(150.0, 90.0)));
}

#[test]
fn a_move_of_a_secondary_window_is_remembered_under_its_own_kind() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(crate::shortcuts::Action::Settings));
    let settings_id = harness.settings_id().expect("settings window is open");

    harness.press(Message::WindowMoveSettled(settings_id, Point::new(7.0, 8.0), false));

    let memory = harness.app.windows.memory();
    let settings = memory.applied_to(WindowKind::Settings, crate::main_window::settings());
    let main = memory.applied_to(WindowKind::Main, crate::main_window::settings());
    assert!(matches!(settings.position, window::Position::Specific(p) if p == Point::new(7.0, 8.0)));
    assert!(matches!(main.position, window::Position::Default));
}

#[test]
fn a_move_of_an_unknown_window_is_ignored() {
    let mut harness = Harness::new();

    harness.press(Message::WindowMoveSettled(window::Id::unique(), Point::new(1.0, 1.0), false));

    assert_eq!(harness.app.windows.memory(), &crate::window_memory::WindowMemory::default());
}

#[test]
fn a_resize_of_a_secondary_window_is_remembered() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(crate::shortcuts::Action::Settings));
    let settings_id = harness.settings_id().expect("settings window is open");

    harness.press(Message::WindowSizeSettled(settings_id, Size::new(640.0, 480.0), false));

    let settings = harness.app.windows.memory().applied_to(WindowKind::Settings, crate::main_window::settings());
    assert_eq!(settings.size, Size::new(640.0, 480.0));
}

#[test]
fn the_remembered_geometry_survives_the_main_window_being_registered() {
    let mut memory = crate::window_memory::WindowMemory::default();
    memory.resized(WindowKind::Search, Size::new(777.0, 555.0));
    let harness = Harness::with_window_memory(memory);

    let settings = harness.app.windows.memory().applied_to(WindowKind::Search, crate::main_window::settings());
    assert_eq!(settings.size, Size::new(777.0, 555.0));
}

#[test]
fn a_search_window_destroyed_without_a_close_request_is_forgotten() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Search));
    let id = harness.search_id().unwrap();

    harness.press(Message::WindowClosed(id));

    assert!(harness.app.search_state().is_none());
    assert!(harness.app.window_id_of(WindowKind::Search).is_none());
}

#[test]
fn a_settings_window_destroyed_without_a_close_request_is_forgotten() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));
    let id = harness.app.window_id_of(WindowKind::Settings).unwrap();

    harness.press(Message::WindowClosed(id));

    assert!(harness.app.window_id_of(WindowKind::Settings).is_none());
}

#[test]
fn a_connections_window_destroyed_without_a_close_request_closes_the_manager() {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);
    let id = harness.connections_id().unwrap();

    harness.press(Message::WindowClosed(id));

    assert!(!harness.app.manager_open());
    assert!(harness.app.window_id_of(WindowKind::Connections).is_none());
}

#[test]
fn the_destruction_of_the_main_window_or_an_unknown_one_changes_nothing() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Search));
    let main = harness.app.main_window();

    harness.press(Message::WindowClosed(main));
    harness.press(Message::WindowClosed(window::Id::unique()));

    assert!(harness.app.search_state().is_some());
    assert!(harness.app.window_id_of(WindowKind::Main).is_some());
}

#[test]
fn a_resize_is_checked_for_maximization_before_it_is_remembered() {
    let mut harness = Harness::new();
    let main = harness.main_id();

    let units = harness.app.update(Message::WindowSized(main, Size::new(1300.0, 800.0))).units();

    assert_eq!(units, 1);
    let remembered = harness.app.windows.memory().applied_to(WindowKind::Main, crate::main_window::settings());
    assert_eq!(remembered.size, crate::main_window::settings().size);
}

#[test]
fn a_move_is_checked_for_maximization_before_it_is_remembered() {
    let mut harness = Harness::new();
    let main = harness.main_id();

    let units = harness.app.update(Message::WindowMoved(main, Point::new(10.0, 10.0))).units();

    assert_eq!(units, 1);
    assert_eq!(harness.app.windows.memory(), &crate::window_memory::WindowMemory::default());
}

#[test]
fn an_unknown_window_is_not_checked() {
    let mut harness = Harness::new();

    let sized = harness.app.update(Message::WindowSized(window::Id::unique(), Size::new(1.0, 1.0))).units();
    let moved = harness.app.update(Message::WindowMoved(window::Id::unique(), Point::new(1.0, 1.0))).units();

    assert_eq!((sized, moved), (0, 0));
}

#[test]
fn maximizing_the_main_window_keeps_the_size_and_position_it_had_before() {
    let mut harness = Harness::new();
    let main = harness.main_id();
    harness.press(Message::WindowSizeSettled(main, Size::new(900.0, 640.0), false));
    harness.press(Message::WindowMoveSettled(main, Point::new(40.0, 50.0), false));

    harness.press(Message::WindowSizeSettled(main, Size::new(1920.0, 1080.0), true));
    harness.press(Message::WindowMoveSettled(main, Point::new(0.0, 0.0), true));

    let settings = harness.app.windows.memory().applied_to(WindowKind::Main, crate::main_window::settings());
    assert!(settings.maximized);
    assert_eq!(settings.size, Size::new(900.0, 640.0));
    assert!(matches!(settings.position, window::Position::Specific(p) if p == Point::new(40.0, 50.0)));
}

#[test]
fn restoring_the_main_window_forgets_the_maximized_state_and_remembers_the_new_size() {
    let mut harness = Harness::new();
    let main = harness.main_id();
    harness.press(Message::WindowSizeSettled(main, Size::new(1920.0, 1080.0), true));

    harness.press(Message::WindowSizeSettled(main, Size::new(1000.0, 700.0), false));

    let settings = harness.app.windows.memory().applied_to(WindowKind::Main, crate::main_window::settings());
    assert!(!settings.maximized);
    assert_eq!(settings.size, Size::new(1000.0, 700.0));
}

#[test]
fn the_live_window_size_follows_every_resize_even_while_maximized() {
    let mut harness = Harness::new();
    let main = harness.main_id();

    harness.press(Message::WindowSized(main, Size::new(1920.0, 1080.0)));

    assert_eq!(harness.app.windows.size_of(main), Some(Size::new(1920.0, 1080.0)));
}

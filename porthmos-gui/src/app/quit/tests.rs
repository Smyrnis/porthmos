use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::Event;
use porthmos_core::transfer::rows::{QueueRow, RowKind, RowState};
use porthmos_core::transfer::{Direction, TransferSnapshot};

use crate::modal::{Modal, quit_text};
use crate::shortcuts::KeyPress;
use crate::testing::Harness;

use super::super::*;

const EDIT_TEXT: &str = "An edit is still being saved to the server. Quit anyway? The remote file may be left incomplete; your copy stays in the state folder.";
const TRANSFER_TEXT: &str = porthmos_core::transfer::QUIT_WITH_TRANSFERS_TEXT;

fn key(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    let text = match &key {
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    };
    harness.press(Message::KeyPressed(KeyPress { key: key.clone(), modified_key: key, modifiers, text }));
}

fn running_transfer(harness: &mut Harness, state: RowState) {
    let row = QueueRow {
        kind: RowKind::Single(1),
        label: "a".to_string(),
        direction: Direction::Upload,
        files_done: 0,
        files_total: 1,
        bytes_done: 10,
        bytes_total: 100,
        state,
        paused: false,
        session_id: Some(1),
        failure: None,
    };
    harness.feed(Event::TransfersChanged(TransferSnapshot { rows: vec![row], ..TransferSnapshot::default() }));
}

#[test]
fn quitting_with_nothing_running_closes_at_once() {
    let mut harness = Harness::new();

    harness.press(Message::CloseRequested);

    assert!(harness.app.is_closing());
    assert!(harness.app.modal().is_none());
}

#[test]
fn a_running_transfer_asks_first() {
    let mut harness = Harness::new();
    running_transfer(&mut harness, RowState::Running);

    harness.press(Message::CloseRequested);

    assert!(!harness.app.is_closing());
    assert!(matches!(harness.app.modal(), Some(Modal::ConfirmQuit { transfers: true, edits: false })));
}

#[test]
fn a_finished_transfer_does_not_ask() {
    let mut harness = Harness::new();
    running_transfer(&mut harness, RowState::Done);

    harness.press(Message::CloseRequested);

    assert!(harness.app.is_closing());
}

#[test]
fn a_busy_edit_asks_first() {
    let mut harness = Harness::new();
    harness.feed(Event::EditsBusy(true));

    harness.press(Message::CloseRequested);

    assert!(matches!(harness.app.modal(), Some(Modal::ConfirmQuit { transfers: false, edits: true })));
}

#[test]
fn both_reasons_are_reported_together() {
    let mut harness = Harness::new();
    harness.feed(Event::EditsBusy(true));
    running_transfer(&mut harness, RowState::Running);

    harness.press(Message::CloseRequested);

    assert!(matches!(harness.app.modal(), Some(Modal::ConfirmQuit { transfers: true, edits: true })));
}

#[test]
fn the_texts_match_the_terminal_front_end() {
    assert_eq!(quit_text(false, true), EDIT_TEXT);
    assert_eq!(quit_text(true, false), TRANSFER_TEXT);
    assert_eq!(quit_text(true, true), format!("{EDIT_TEXT}\n\n{TRANSFER_TEXT}"));
}

#[test]
fn accepting_closes_and_declining_does_not() {
    let mut harness = Harness::new();
    harness.feed(Event::EditsBusy(true));
    harness.press(Message::CloseRequested);
    harness.press(Message::ModalDeclined);
    assert!(!harness.app.is_closing());
    assert!(harness.app.modal().is_none());

    harness.press(Message::CloseRequested);
    harness.press(Message::ModalAccepted);

    assert!(harness.app.is_closing());
    assert!(harness.app.modal().is_none());
}

#[test]
fn enter_and_escape_both_keep_the_app_open_while_work_is_running() {
    let mut harness = Harness::new();
    harness.feed(Event::EditsBusy(true));
    harness.press(Message::CloseRequested);
    key(&mut harness, Key::Named(Named::Escape), Modifiers::empty());
    assert!(!harness.app.is_closing());

    harness.press(Message::CloseRequested);
    key(&mut harness, Key::Named(Named::Enter), Modifiers::empty());

    assert!(!harness.app.is_closing());
    assert!(harness.app.modal().is_none());
}

#[test]
fn the_quit_shortcut_goes_through_the_same_guard() {
    let mut harness = Harness::new();
    running_transfer(&mut harness, RowState::Running);

    key(&mut harness, Key::Character("q".into()), Modifiers::COMMAND);

    assert!(!harness.app.is_closing());
    assert!(matches!(harness.app.modal(), Some(Modal::ConfirmQuit { .. })));
}

#[test]
fn asking_twice_shows_one_question() {
    let mut harness = Harness::new();
    harness.feed(Event::EditsBusy(true));

    harness.press(Message::CloseRequested);
    harness.press(Message::CloseRequested);
    harness.press(Message::ModalDeclined);

    assert!(harness.app.modal().is_none());
}

#[test]
fn a_close_request_while_closing_is_ignored() {
    let mut harness = Harness::new();
    harness.press(Message::CloseRequested);
    assert!(harness.app.is_closing());

    harness.press(Message::CloseRequested);

    assert!(harness.app.modal().is_none());
}

#[test]
fn closing_with_no_secondary_window_open_starts_only_the_shutdown() {
    let mut harness = Harness::new();

    let units = harness.app.update(Message::CloseRequested).units();

    assert_eq!(units, 1);
}

#[test]
fn closing_the_main_window_with_secondary_windows_open_also_closes_them() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));
    harness.press(Message::ManagerToggled);
    assert!(harness.settings_id().is_some());
    assert!(harness.connections_id().is_some());
    let main = harness.main_id();

    let units = harness.app.update(Message::WindowCloseRequested(main)).units();

    assert!(harness.app.is_closing());
    assert_eq!(units, 3);
}

#[test]
fn the_quit_confirmation_raises_the_main_window() {
    let mut harness = Harness::new();
    running_transfer(&mut harness, RowState::Running);
    harness.press(Message::ManagerToggled);
    let main = harness.main_id();

    let units = harness.app.update(Message::WindowCloseRequested(main)).units();

    assert!(matches!(harness.app.modal(), Some(Modal::ConfirmQuit { .. })));
    assert_eq!(units, 1);
}

#[test]
fn asking_twice_does_not_raise_the_main_window_again() {
    let mut harness = Harness::new();
    running_transfer(&mut harness, RowState::Running);
    harness.press(Message::CloseRequested);

    let units = harness.app.update(Message::CloseRequested).units();

    assert_eq!(units, 0);
}

#[test]
fn closing_the_main_window_with_search_open_cancels_the_search_and_closes_its_window() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Search));
    harness.press(Message::SearchTyped("a".to_string()));
    harness.sent();
    let main = harness.main_id();

    let units = harness.app.update(Message::WindowCloseRequested(main)).units();

    assert_eq!(units, 2);
    assert!(harness.sent().contains(&Command::CancelSearch));
}

#[test]
fn quitting_saves_the_window_sizes_and_positions() {
    let mut harness = Harness::with_window_memory(Default::default());
    let main = harness.main_id();
    harness.press(Message::WindowSized(main, iced::Size::new(900.0, 600.0)));
    harness.press(Message::WindowMoved(main, iced::Point::new(25.0, 35.0)));

    harness.press(Message::CloseRequested);

    let saved = crate::window_memory::WindowMemory::load(&harness.window_memory_file()).memory;
    assert_eq!(&saved, harness.app.windows.memory());
    let settings = saved.applied_to(crate::windows::WindowKind::Main, crate::main_window::settings());
    assert_eq!(settings.size, iced::Size::new(900.0, 600.0));
}

#[test]
fn a_secondary_window_closed_earlier_is_still_saved_on_quit() {
    let mut harness = Harness::with_window_memory(Default::default());
    harness.press(Message::ActionRequested(crate::shortcuts::Action::Settings));
    let settings_id = harness.settings_id().expect("settings window is open");
    harness.press(Message::WindowSized(settings_id, iced::Size::new(640.0, 480.0)));
    harness.press(Message::WindowCloseRequested(settings_id));

    harness.press(Message::CloseRequested);

    let saved = crate::window_memory::WindowMemory::load(&harness.window_memory_file()).memory;
    let settings = saved.applied_to(crate::windows::WindowKind::Settings, crate::main_window::settings());
    assert_eq!(settings.size, iced::Size::new(640.0, 480.0));
}

#[test]
fn nothing_is_saved_while_a_quit_is_waiting_for_confirmation() {
    let mut harness = Harness::with_window_memory(Default::default());
    running_transfer(&mut harness, RowState::Running);

    harness.press(Message::CloseRequested);

    assert!(!harness.app.is_closing());
    assert!(!harness.window_memory_file().exists());
}

#[test]
fn a_failed_save_does_not_stop_the_app_from_quitting() {
    let mut harness = Harness::new();
    let blocker = harness.scratch().join("blocker");
    std::fs::write(&blocker, "a file, not a folder").unwrap();
    harness.app.set_window_memory(Default::default(), blocker.join("gui-windows.toml"));

    harness.press(Message::CloseRequested);

    assert!(harness.app.is_closing());
}

use iced::Size;
use iced::keyboard::{Key, Modifiers};

use super::{reset_enabled, shortcut_text};
use crate::app::Message;
use crate::geometry::ROW_HEIGHT;
use crate::settings_view::{GuiSettings, SettingsState};
use crate::shortcuts::{Action, Chord, Shortcuts};
use crate::testing::Harness;
use crate::tools::Tool;
use crate::view::tools::{SETTINGS_CHROME, SETTINGS_HEIGHT, SETTINGS_WIDTH, frame_size, list_height, overlay};

fn settings_tool() -> Tool {
    Tool::Settings(SettingsState::new(&GuiSettings::default()))
}

#[test]
fn the_overlay_builds_with_every_action_row() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));

    assert!(overlay(&harness.app).is_some());
}

#[test]
fn the_overlay_builds_while_capturing_and_with_a_message() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));
    harness.press(Message::SettingsChangePressed(Action::Refresh));

    assert!(overlay(&harness.app).is_some());
}

#[test]
fn reset_is_disabled_for_an_action_with_its_defaults() {
    let mut shortcuts = Shortcuts::default();
    assert!(!reset_enabled(&shortcuts, Action::Refresh));

    shortcuts.assign(Action::Refresh, Chord::new(Key::Character("o".into()), Modifiers::CTRL)).unwrap();

    assert!(reset_enabled(&shortcuts, Action::Refresh));
    assert!(!reset_enabled(&shortcuts, Action::Search));
}

#[test]
fn the_settings_frame_keeps_its_size_in_a_large_window() {
    let window = Size::new(1100.0, 700.0);

    assert_eq!(frame_size(&settings_tool(), window), Size::new(SETTINGS_WIDTH, SETTINGS_HEIGHT));
    assert_eq!(list_height(&settings_tool(), window), SETTINGS_HEIGHT - SETTINGS_CHROME);
}

#[test]
fn the_settings_frame_shrinks_with_the_window_and_keeps_one_row() {
    let small = Size::new(480.0, 320.0);

    assert_eq!(frame_size(&settings_tool(), small), Size::new(480.0 - 32.0, 320.0 - 32.0));
    assert!(list_height(&settings_tool(), small) >= ROW_HEIGHT);
    assert_eq!(list_height(&settings_tool(), Size::new(10.0, 10.0)), ROW_HEIGHT);
}

#[test]
fn shortcut_cells_show_the_chord_labels_and_a_dash_for_unbound() {
    let shortcuts = Shortcuts::default();

    assert_eq!(shortcut_text(&shortcuts, Action::MoveUp), "Up, K");
    assert_eq!(shortcut_text(&shortcuts, Action::Palette), "Ctrl+P");
    assert_eq!(shortcut_text(&shortcuts, Action::CheckForUpdates), "\u{2014}");
}

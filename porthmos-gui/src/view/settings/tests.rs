use iced::Size;
use iced::keyboard::{Key, Modifiers};

use super::{SETTINGS_CHROME, reset_enabled, window_list_height};
use crate::app::Message;
use crate::geometry::ROW_HEIGHT;
use crate::shortcuts::{Action, Chord, Shortcuts};
use crate::testing::Harness;
use porthmos_core::RefreshTab;

#[test]
fn the_window_builds_with_every_action_row() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));

    assert!(super::window(&harness.app).is_some());
}

#[test]
fn the_window_builds_while_capturing_and_with_a_message() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));
    harness.press(Message::SettingsChangePressed(Action::Refresh));

    assert!(super::window(&harness.app).is_some());
}

#[test]
fn there_is_no_settings_window_while_the_settings_are_closed() {
    let harness = Harness::new();

    assert!(super::window(&harness.app).is_none());
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
fn the_list_fills_the_window_below_the_chrome() {
    assert_eq!(window_list_height(Size::new(760.0, 600.0)), 600.0 - SETTINGS_CHROME);
}

#[test]
fn the_list_keeps_one_row_in_a_tiny_window() {
    assert!(window_list_height(Size::new(480.0, 320.0)) >= ROW_HEIGHT);
    assert_eq!(window_list_height(Size::new(10.0, 10.0)), ROW_HEIGHT);
}

#[test]
fn the_window_builds_for_every_category() {
    use crate::settings_view::SettingsCategory;
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));

    for category in SettingsCategory::ALL {
        harness.press(Message::SettingsCategoryChosen(category));
        assert!(super::window(&harness.app).is_some(), "{category:?}");
    }
}

#[test]
fn a_refresh_seconds_field_is_enabled_only_while_its_checkbox_is_ticked() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));

    for tab in [RefreshTab::Health, RefreshTab::Services, RefreshTab::Ports] {
        assert!(!super::refresh_field_enabled(&harness.app, tab), "{tab:?}");
    }

    harness.press(Message::SettingsRefreshToggled(RefreshTab::Services, true));

    assert!(!super::refresh_field_enabled(&harness.app, RefreshTab::Health));
    assert!(super::refresh_field_enabled(&harness.app, RefreshTab::Services));
    assert!(!super::refresh_field_enabled(&harness.app, RefreshTab::Ports));
    assert!(super::window(&harness.app).is_some());
}

#[test]
fn every_refresh_seconds_field_is_labelled_with_its_unit_and_the_window_builds_with_them() {
    let mut harness = Harness::new();
    harness.press(Message::ActionRequested(Action::Settings));

    assert_eq!(super::refresh_unit_label(), "seconds");
    assert!(super::window(&harness.app).is_some());
}

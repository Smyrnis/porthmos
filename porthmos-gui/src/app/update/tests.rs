use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::{Command, Event, Severity, UpdateMode, UpdateState};

use crate::menus::MoreItem;
use crate::modal::Modal;
use crate::shortcuts::{Action, KeyPress};
use crate::testing::Harness;
use crate::update_view::BadgeKind;
use crate::view::toolbar::badge_label;

use super::super::*;

fn ready(version: &str) -> Event {
    Event::Update(UpdateState::Ready { version: version.to_string() })
}

fn notify_harness() -> Harness {
    Harness::with_update_mode(UpdateMode::Notify)
}

fn infos(harness: &Harness) -> Vec<String> {
    harness
        .app
        .notices()
        .iter()
        .filter(|notice| notice.severity == Severity::Info)
        .map(|notice| notice.message.clone())
        .collect()
}

#[test]
fn an_update_event_shows_the_badge_and_a_notice() {
    let mut harness = Harness::new();

    harness.feed(ready("0.4.0"));

    let badge = harness.app.update_badge().expect("a ready update has a badge");
    assert_eq!((badge.version.as_str(), badge.kind), ("0.4.0", BadgeKind::Ready));
    assert_eq!(infos(&harness), ["Porthmos 0.4.0 is ready and will be installed when you quit"]);
    harness.feed(ready("0.4.0"));
    assert_eq!(infos(&harness).len(), 1);
}

#[test]
fn pressing_the_badge_opens_the_announcement() {
    let mut harness = Harness::new();
    harness.feed(ready("0.4.0"));

    harness.press(Message::UpdateBadgePressed);

    match harness.app.modal() {
        Some(Modal::Info { title, body }) => {
            assert_eq!(title, "Update");
            assert_eq!(body, "Porthmos 0.4.0 is ready and will be installed when you quit");
        }
        other => panic!("expected the update dialog, got {other:?}"),
    }
}

#[test]
fn pressing_the_badge_without_an_update_does_nothing() {
    let mut harness = Harness::new();

    harness.press(Message::UpdateBadgePressed);

    assert!(harness.app.modal().is_none());
}

#[test]
fn check_for_updates_without_a_badge_sends_one_manual_check() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::ActionRequested(Action::CheckForUpdates));

    assert_eq!(harness.sent(), [Command::CheckForUpdate { manual: true }]);
}

#[test]
fn check_for_updates_with_a_badge_announces_instead() {
    let mut harness = Harness::new();
    harness.feed(ready("0.4.0"));
    harness.sent();

    harness.press(Message::ActionRequested(Action::CheckForUpdates));

    assert!(harness.sent().is_empty());
    assert_eq!(infos(&harness).len(), 2);
}

#[test]
fn the_more_item_and_the_palette_command_check_for_updates() {
    let mut harness = Harness::new();
    harness.sent();
    harness.press(Message::MoreMenuToggled);
    harness.press(Message::MoreItemChosen(MoreItem::CheckForUpdates));
    assert_eq!(harness.sent(), [Command::CheckForUpdate { manual: true }]);

    harness.press(Message::ActionRequested(Action::Palette));
    harness.sent();
    harness.press(Message::PaletteTyped(">check for updates".to_string()));
    let key = Key::Named(Named::Enter);
    harness.press(Message::KeyPressed(KeyPress {
        key: key.clone(),
        modified_key: key,
        modifiers: Modifiers::empty(),
        text: None,
    }));

    assert_eq!(harness.sent(), [Command::CheckForUpdate { manual: true }]);
    assert!(harness.app.tool().is_none());
}

#[test]
fn the_badge_label_is_absent_without_an_update_and_follows_the_window_width() {
    let mut harness = notify_harness();
    assert_eq!(badge_label(&harness.app), None);

    harness.feed(Event::Update(UpdateState::Available { version: "0.4.0".to_string() }));
    assert_eq!(badge_label(&harness.app).as_deref(), Some("\u{2191} 0.4.0 available"));

    harness.press(Message::WindowResized(iced::Size::new(700.0, 500.0)));
    assert_eq!(badge_label(&harness.app).as_deref(), Some("\u{2191} 0.4.0"));
    harness.press(Message::WindowResized(iced::Size::new(600.0, 500.0)));
    assert_eq!(badge_label(&harness.app).as_deref(), Some("\u{2191}"));
}

#[test]
fn available_in_auto_mode_has_no_badge_because_core_downloads_it() {
    let mut harness = Harness::new();

    harness.feed(Event::Update(UpdateState::Available { version: "0.4.0".to_string() }));

    assert_eq!(harness.app.update_badge(), None);
    assert!(infos(&harness).is_empty());
}

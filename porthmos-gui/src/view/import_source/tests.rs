use porthmos_core::import::ImportSource;

use super::{browse_labels, read_enabled, source_title};
use crate::app::Message;
use crate::testing::Harness;
use crate::view::connection_form::overlay;

#[test]
fn the_source_modal_renders_idle_with_an_error_and_while_reading() {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);
    harness.press(Message::ImportPressed);
    assert!(overlay(&harness.app).is_some());

    harness.press(Message::ImportPathTyped(String::new()));
    harness.press(Message::ImportReadPressed);
    assert!(overlay(&harness.app).is_some());

    harness.press(Message::ImportPathTyped("/tmp/a.xml".to_string()));
    harness.press(Message::ImportReadPressed);
    assert!(overlay(&harness.app).is_some());
}

#[test]
fn the_read_button_is_off_while_a_read_is_running() {
    assert!(read_enabled(None));
    assert!(!read_enabled(Some(3)));
}

#[test]
fn each_source_is_offered_by_its_label() {
    let titles: Vec<&str> = ImportSource::ALL.into_iter().map(source_title).collect();

    assert_eq!(titles, ["FileZilla", "WinSCP", "PuTTY"]);
}

#[test]
fn putty_offers_a_file_and_a_folder_while_the_others_offer_one_file_button() {
    assert_eq!(browse_labels(ImportSource::FileZilla), vec![("Browse\u{2026}", false)]);
    assert_eq!(browse_labels(ImportSource::WinScp), vec![("Browse\u{2026}", false)]);
    assert_eq!(browse_labels(ImportSource::Putty), vec![("File\u{2026}", false), ("Folder\u{2026}", true)]);
}

#[test]
fn cancel_leaves_the_source_screen_and_cancel_on_the_review_is_its_own_message() {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);
    harness.press(Message::ImportPressed);

    harness.press(super::cancel_message());

    assert!(matches!(harness.app.screen(), Some(crate::connections::Screen::List)));
}

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::properties::{Digest, Measured};
use porthmos_core::{Command, Entry, Event, Location, VfsPath};

use crate::app::Message;
use crate::modal::Modal;
use crate::modal::properties::{HashState, MeasurePane, PropertiesPane};
use crate::shortcuts::{Action, KeyPress};
use crate::testing::Harness;

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn file(folder: &VfsPath, name: &str, is_dir: bool) -> Entry {
    Entry { name: name.to_string(), path: folder.join(name), is_dir, size: 10, ..Entry::default() }
}

fn harness_with(entries: &[(&str, bool)]) -> Harness {
    let mut harness = Harness::new();
    let folder = harness.start_folder();
    let entries = entries.iter().map(|(name, is_dir)| file(&folder, name, *is_dir)).collect();
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries });
    harness.sent();
    harness
}

fn press_key(harness: &mut Harness, key: Key, modifiers: Modifiers) {
    harness.press(Message::KeyPressed(KeyPress { key: key.clone(), modified_key: key, modifiers, text: None }));
}

fn plain(harness: &mut Harness, named: Named) {
    press_key(harness, Key::Named(named), Modifiers::empty());
}

fn select_all_entries(harness: &mut Harness) {
    plain(harness, Named::ArrowDown);
    plain(harness, Named::Space);
    plain(harness, Named::ArrowDown);
    plain(harness, Named::Space);
}

fn open(harness: &mut Harness) {
    harness.press(Message::ActionRequested(Action::Properties));
}

fn pane(harness: &Harness) -> &PropertiesPane {
    match harness.app.modal() {
        Some(Modal::Properties { pane, .. }) => pane,
        other => panic!("{other:?}"),
    }
}

fn measured(bytes: u64, files: u64) -> Measured {
    Measured { bytes, files, ..Measured::default() }
}

fn digest() -> Digest {
    Digest { hex: ABC.to_string(), bytes: 3 }
}

#[test]
fn the_action_opens_the_modal_and_measures_the_entry_under_the_cursor() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);

    open(&mut harness);

    assert!(matches!(harness.app.modal(), Some(Modal::Properties { request_id: 1, .. })));
    assert_eq!(
        harness.sent(),
        vec![Command::Measure {
            request_id: 1,
            location: Location::Local,
            paths: vec![harness.start_folder().join("a.iso")]
        }]
    );
    assert_eq!(pane(&harness).title, "Properties: a.iso");
}

#[test]
fn alt_enter_opens_it_from_the_keyboard() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);

    press_key(&mut harness, Key::Named(Named::Enter), Modifiers::ALT);

    assert!(matches!(harness.app.modal(), Some(Modal::Properties { .. })));
}

#[test]
fn the_parent_row_alone_does_nothing() {
    let mut harness = harness_with(&[("a.iso", false)]);

    open(&mut harness);

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn a_selection_is_measured_as_a_whole_and_only_files_are_checksummed() {
    let mut harness = harness_with(&[("a.iso", false), ("b", true)]);
    select_all_entries(&mut harness);
    open(&mut harness);

    let sent = harness.sent();
    let Some(Command::Measure { paths, request_id: 1, .. }) = sent.first() else { panic!("{sent:?}") };
    assert_eq!(paths.len(), 2);
    assert_eq!(pane(&harness).title, "Properties: 2 items");
    assert_eq!(pane(&harness).files.len(), 1);

    harness.press(Message::PropertiesCompute);

    assert_eq!(
        harness.sent(),
        vec![Command::Checksum {
            request_id: 1,
            location: Location::Local,
            paths: vec![harness.start_folder().join("a.iso")]
        }]
    );
}

#[test]
fn a_second_open_gets_a_new_request_id() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);
    harness.press(Message::ModalDeclined);
    harness.press(Message::ModalDeclined);
    harness.sent();

    open(&mut harness);

    assert!(matches!(harness.sent().first(), Some(Command::Measure { request_id: 2, .. })));
}

#[test]
fn a_selection_of_folders_only_has_nothing_to_checksum() {
    let mut harness = harness_with(&[("b", true)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);
    harness.sent();

    harness.press(Message::PropertiesCompute);

    assert!(harness.sent().is_empty());
}

#[test]
fn measure_events_update_the_open_modal() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);

    harness.feed(Event::MeasureProgress { request_id: 1, measured: measured(0, 812) });
    assert_eq!(pane(&harness).measure, MeasurePane::Running(measured(0, 812)));
    harness.feed(Event::Measured { request_id: 1, result: Ok(measured(10, 1)) });

    assert_eq!(pane(&harness).measure, MeasurePane::Done(measured(10, 1)));
}

#[test]
fn checksum_events_update_the_open_modal() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);
    harness.press(Message::PropertiesCompute);
    let path = harness.start_folder().join("a.iso");

    harness.feed(Event::ChecksumProgress { request_id: 1, path: path.clone(), bytes_done: 5, bytes_total: 10 });
    assert_eq!(pane(&harness).files[0].state, HashState::Running { done: 5, total: 10 });
    harness.feed(Event::Checksummed { request_id: 1, path, result: Ok(digest()) });

    assert_eq!(pane(&harness).files[0].state, HashState::Done(digest()));
}

#[test]
fn events_for_another_request_or_with_no_modal_are_ignored() {
    let mut harness = harness_with(&[("a.iso", false)]);
    harness.feed(Event::Measured { request_id: 9, result: Ok(measured(1, 1)) });
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);

    harness.feed(Event::Measured { request_id: 9, result: Ok(measured(7_000, 1)) });

    assert_eq!(pane(&harness).measure, MeasurePane::Running(Measured::default()));
}

#[test]
fn an_event_finds_its_modal_even_when_another_modal_is_in_front() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    harness.app.modals.push(Modal::Info { title: "Notice".to_string(), body: "x".to_string() });
    harness.app.open_properties(crate::app::PanelSide::Local);
    assert!(matches!(harness.app.modal(), Some(Modal::Info { .. })));

    harness.feed(Event::Measured { request_id: 1, result: Ok(measured(10, 1)) });
    harness.app.modals.pop();

    assert_eq!(pane(&harness).measure, MeasurePane::Done(measured(10, 1)));
}

#[test]
fn the_first_decline_cancels_a_running_job_and_the_second_closes() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);
    harness.sent();

    harness.press(Message::ModalDeclined);
    assert_eq!(pane(&harness).measure, MeasurePane::Cancelled);
    assert!(harness.app.modal().is_some());
    assert_eq!(harness.sent(), vec![Command::CancelProperties { request_id: 1 }]);

    harness.press(Message::ModalDeclined);
    assert!(harness.app.modal().is_none());
    assert_eq!(harness.sent(), vec![Command::CancelProperties { request_id: 1 }]);
}

#[test]
fn escape_behaves_like_the_cancel_button() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);
    harness.press(Message::PropertiesCompute);
    harness.feed(Event::Measured { request_id: 1, result: Ok(measured(10, 1)) });
    harness.sent();

    plain(&mut harness, Named::Escape);
    assert_eq!(pane(&harness).files[0].state, HashState::Cancelled);
    plain(&mut harness, Named::Escape);

    assert!(harness.app.modal().is_none());
}

#[test]
fn enter_does_not_close_or_accept_the_modal() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);

    plain(&mut harness, Named::Enter);

    assert!(matches!(harness.app.modal(), Some(Modal::Properties { .. })));
}

#[test]
fn the_expected_text_is_kept_and_a_wrong_copy_index_is_harmless() {
    let mut harness = harness_with(&[("a.iso", false)]);
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);

    harness.press(Message::PropertiesExpectedTyped("abc".to_string()));
    harness.press(Message::PropertiesCopyDigest(7));

    assert_eq!(pane(&harness).expected, "abc");
}

#[test]
fn calculate_goes_to_the_server_the_dialog_was_opened_on_even_if_another_becomes_active() {
    let mut harness = harness_with(&[("a.iso", false)]);
    harness.feed(Event::Connected { session: 1, name: "alpha".to_string(), shell_available: false });
    let folder = VfsPath::new("/srv");
    harness.feed(Event::Listed {
        location: Location::Session(1),
        path: folder.clone(),
        entries: vec![file(&folder, "big.iso", false)],
    });
    plain(&mut harness, Named::ArrowDown);
    open(&mut harness);
    assert!(matches!(harness.sent().last(), Some(Command::Measure { location: Location::Session(1), .. })));

    harness.feed(Event::Connected { session: 2, name: "beta".to_string(), shell_available: false });
    harness.press(Message::PropertiesCompute);

    assert_eq!(
        harness.sent().last(),
        Some(&Command::Checksum { request_id: 1, location: Location::Session(1), paths: vec![folder.join("big.iso")] })
    );
}

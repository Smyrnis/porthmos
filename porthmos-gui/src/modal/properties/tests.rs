use porthmos_core::VfsPath;
use porthmos_core::properties::{Digest, Measured};

use super::*;

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn pane_for(names: &[&str]) -> PropertiesPane {
    PropertiesPane::new(
        "Properties".to_string(),
        names.iter().map(|name| (VfsPath::new(&format!("/d/{name}")), name.to_string())).collect(),
    )
}

fn path(name: &str) -> VfsPath {
    VfsPath::new(&format!("/d/{name}"))
}

fn digest() -> Digest {
    Digest { hex: ABC.to_string(), bytes: 3 }
}

fn measured() -> Measured {
    Measured { bytes: 10, files: 1, ..Measured::default() }
}

#[test]
fn a_new_pane_is_measuring_and_nothing_is_hashing() {
    let pane = pane_for(&["a.iso"]);

    assert!(pane.measuring());
    assert!(!pane.hashing());
    assert!(pane.running());
    assert_eq!(pane.measure, MeasurePane::Running(Measured::default()));
}

#[test]
fn progress_then_the_result_replace_the_measure_state() {
    let mut pane = pane_for(&[]);

    pane.set_progress(Measured { files: 5, ..Measured::default() });
    assert_eq!(pane.measure, MeasurePane::Running(Measured { files: 5, ..Measured::default() }));
    pane.set_measured(Ok(measured()));

    assert_eq!(pane.measure, MeasurePane::Done(measured()));
    assert!(!pane.running());
    pane.set_progress(Measured { files: 9, ..Measured::default() });
    assert_eq!(pane.measure, MeasurePane::Done(measured()));
}

#[test]
fn a_failed_measure_keeps_its_text() {
    let mut pane = pane_for(&[]);

    pane.set_measured(Err("Could not measure the selection:\nreset".into()));

    assert_eq!(pane.measure, MeasurePane::Failed("Could not measure the selection:\nreset".into()));
}

#[test]
fn a_late_result_does_not_undo_a_cancelled_measure() {
    let mut pane = pane_for(&[]);

    pane.cancel_running();
    pane.set_measured(Ok(measured()));

    assert_eq!(pane.measure, MeasurePane::Cancelled);
}

#[test]
fn computing_marks_every_file_running_and_returns_their_paths_once() {
    let mut pane = pane_for(&["a.iso", "b.iso"]);
    pane.set_measured(Ok(measured()));

    assert!(pane.can_compute());
    let paths = pane.begin_checksums();

    assert_eq!(paths, Some(vec![path("a.iso"), path("b.iso")]));
    assert!(pane.hashing());
    assert!(!pane.can_compute());
    assert_eq!(pane.begin_checksums(), None);
}

#[test]
fn computing_needs_at_least_one_file() {
    let mut pane = pane_for(&[]);

    assert!(!pane.can_compute());
    assert_eq!(pane.begin_checksums(), None);
}

#[test]
fn a_hash_can_be_started_again_once_every_file_has_an_answer() {
    let mut pane = pane_for(&["a.iso"]);
    pane.begin_checksums();
    pane.set_checksum(&path("a.iso"), Ok(digest()));

    assert!(pane.can_compute());
    assert_eq!(pane.begin_checksums(), Some(vec![path("a.iso")]));
}

#[test]
fn checksum_progress_and_results_find_their_file_by_path() {
    let mut pane = pane_for(&["a.iso", "b.iso"]);
    pane.begin_checksums();

    pane.set_checksum_progress(&path("b.iso"), 5, 10);
    pane.set_checksum(&path("a.iso"), Ok(digest()));

    assert_eq!(pane.files[1].state, HashState::Running { done: 5, total: 10 });
    assert_eq!(pane.files[1].listed_total, Some(10));
    assert_eq!(pane.files[0].state, HashState::Done(digest()));
    pane.set_checksum(&path("nope"), Ok(digest()));
}

#[test]
fn cancelling_marks_running_hashes_cancelled_and_a_late_result_is_ignored() {
    let mut pane = pane_for(&["a.iso"]);
    pane.set_measured(Ok(measured()));
    pane.begin_checksums();

    pane.cancel_running();
    pane.set_checksum(&path("a.iso"), Ok(digest()));

    assert_eq!(pane.files[0].state, HashState::Cancelled);
    assert!(!pane.running());
}

#[test]
fn cancelling_leaves_finished_digests_alone() {
    let mut pane = pane_for(&["a.iso", "b.iso"]);
    pane.begin_checksums();
    pane.set_checksum(&path("a.iso"), Ok(digest()));

    pane.cancel_running();

    assert_eq!(pane.files[0].state, HashState::Done(digest()));
    assert_eq!(pane.files[1].state, HashState::Cancelled);
}

#[test]
fn the_expected_field_exists_for_exactly_one_file() {
    assert!(pane_for(&["a.iso"]).has_expected_field());
    assert!(!pane_for(&["a.iso", "b.iso"]).has_expected_field());
    assert!(!pane_for(&[]).has_expected_field());
}

#[test]
fn the_verdict_needs_a_digest_and_an_expected_value() {
    let mut pane = pane_for(&["a.iso"]);
    pane.expected = ABC.to_string();
    assert_eq!(pane.verdict(), None);

    pane.begin_checksums();
    pane.set_checksum(&path("a.iso"), Ok(digest()));
    assert_eq!(pane.verdict(), Some(true));

    pane.expected = format!("{}  a.iso", ABC.to_uppercase());
    assert_eq!(pane.verdict(), Some(true));
    pane.expected = "0".repeat(64);
    assert_eq!(pane.verdict(), Some(false));
    pane.expected = "   ".to_string();
    assert_eq!(pane.verdict(), None);
}

#[test]
fn there_is_no_verdict_for_several_files() {
    let mut pane = pane_for(&["a.iso", "b.iso"]);
    pane.expected = ABC.to_string();
    pane.begin_checksums();
    pane.set_checksum(&path("a.iso"), Ok(digest()));
    pane.set_checksum(&path("b.iso"), Ok(digest()));

    assert_eq!(pane.verdict(), None);
}

#[test]
fn only_a_finished_digest_can_be_copied() {
    let mut pane = pane_for(&["a.iso", "b.iso"]);
    pane.begin_checksums();
    pane.set_checksum(&path("a.iso"), Ok(digest()));
    pane.set_checksum(&path("b.iso"), Err("x".into()));

    assert_eq!(pane.digest_text(0), Some(ABC.to_string()));
    assert_eq!(pane.digest_text(1), None);
    assert_eq!(pane.digest_text(7), None);
}

#[test]
fn a_failed_checksum_keeps_its_summary_and_cause() {
    let mut pane = pane_for(&["a.iso"]);
    pane.set_measured(Ok(measured()));
    pane.begin_checksums();
    let message = porthmos_core::Message::with_cause("Could not read the file", &"Permission denied");

    pane.set_checksum(&path("a.iso"), Err(message.clone()));

    assert_eq!(pane.files[0].state, HashState::Failed(message));
}

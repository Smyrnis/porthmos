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

#[test]
fn a_finished_measure_has_size_contents_and_skipped_rows() {
    let mut pane = pane_for(&[]);
    pane.set_measured(Ok(Measured { bytes: 4_521_983_744, files: 1_204, folders: 87, symlinks: 2, unreadable: 1 }));

    assert_eq!(
        measure_rows(&pane),
        vec![
            ("Size".to_string(), "4.2 GB (4,521,983,744 bytes)".to_string()),
            ("Contents".to_string(), "1,204 files, 87 folders".to_string()),
            ("Skipped".to_string(), "2 links not followed, 1 folder not readable".to_string()),
        ]
    );
}

#[test]
fn a_measure_with_nothing_skipped_has_no_skipped_row() {
    let mut pane = pane_for(&[]);
    pane.set_measured(Ok(Measured { bytes: 5, files: 1, ..Measured::default() }));

    let labels: Vec<String> = measure_rows(&pane).into_iter().map(|(label, _)| label).collect();

    assert_eq!(labels, ["Size", "Contents"]);
}

#[test]
fn a_running_measure_shows_the_size_so_far_and_progress() {
    let mut pane = pane_for(&[]);
    pane.set_progress(Measured { bytes: 2_048, files: 812, folders: 40, ..Measured::default() });

    assert_eq!(
        measure_rows(&pane),
        vec![
            ("Size".to_string(), "2.0 KB (2,048 bytes)".to_string()),
            ("Progress".to_string(), "measuring\u{2026} 812 files and 40 folders so far".to_string()),
        ]
    );
}

#[test]
fn a_failed_measure_shows_one_line_of_error_and_no_numbers() {
    let mut pane = pane_for(&[]);
    pane.set_measured(Err("Could not measure the selection:\nConnection\u{1b}[2J reset".to_string()));

    let rows = measure_rows(&pane);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "Error");
    assert_eq!(rows[0].1, "Could not measure the selection: Connection?[2J reset");
}

#[test]
fn a_cancelled_measure_says_cancelled_not_zero() {
    let mut pane = pane_for(&[]);
    pane.cancel_running();

    assert_eq!(measure_rows(&pane), vec![("Size".to_string(), "cancelled".to_string())]);
}

#[test]
fn the_largest_size_is_spelled_out() {
    let mut pane = pane_for(&[]);
    pane.set_measured(Ok(Measured { bytes: u64::MAX, ..Measured::default() }));

    assert!(measure_rows(&pane)[0].1.ends_with("(18,446,744,073,709,551,615 bytes)"));
}

#[test]
fn file_names_are_printable_and_clipped() {
    let pane = PropertiesPane::new(
        "p".to_string(),
        vec![(VfsPath::new("/d/x"), format!("bad\u{1b}[31m\n\u{202e}{}", "n".repeat(200)))],
    );

    let name = file_name(&pane.files[0]);

    assert!(!name.chars().any(|character| character.is_control() || character == '\u{202e}'), "{name:?}");
    assert!(name.chars().count() <= 60, "{}", name.chars().count());
}

#[test]
fn a_file_status_tells_each_state() {
    let mut pane = pane_for(&["a.iso", "b.iso", "c.iso", "d.iso", "e.iso"]);
    assert_eq!(file_status(&pane.files[0]), "not computed");

    pane.begin_checksums();
    assert_eq!(file_status(&pane.files[0]), "reading\u{2026} 0 B");
    pane.set_checksum_progress(&path("a.iso"), 2_048, 4_096);
    assert_eq!(file_status(&pane.files[0]), "2.0 KB of 4.0 KB");
    pane.set_checksum(&path("b.iso"), Err("Could not read the file:\nPermission denied".to_string()));
    assert_eq!(file_status(&pane.files[1]), "Could not read the file: Permission denied");
    pane.set_checksum(&path("c.iso"), Ok(digest()));
    assert_eq!(file_status(&pane.files[2]), "");
    pane.set_checksum_progress(&path("d.iso"), 3, 5);
    pane.set_checksum(&path("d.iso"), Ok(digest()));
    assert_eq!(file_status(&pane.files[3]), "3 bytes read, the listing said 5");
    pane.cancel_running();
    assert_eq!(file_status(&pane.files[4]), "cancelled");
}

#[test]
fn the_verdict_line_is_a_word_and_a_flag() {
    let mut pane = pane_for(&["a.iso"]);
    pane.begin_checksums();
    pane.set_checksum(&path("a.iso"), Ok(digest()));
    assert_eq!(verdict_line(&pane), None);

    pane.expected = ABC.to_string();
    assert_eq!(verdict_line(&pane), Some(("matches".to_string(), true)));
    pane.expected = "0".repeat(64);
    assert_eq!(verdict_line(&pane), Some(("does NOT match".to_string(), false)));
}

#[test]
fn the_close_button_reads_cancel_while_anything_runs() {
    let mut pane = pane_for(&["a.iso"]);
    assert_eq!(close_label(&pane), "Cancel");

    pane.set_measured(Ok(Measured::default()));
    assert_eq!(close_label(&pane), "Close");
    pane.begin_checksums();
    assert_eq!(close_label(&pane), "Cancel");
}

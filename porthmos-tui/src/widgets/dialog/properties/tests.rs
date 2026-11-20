use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::VfsPath;
use porthmos_core::properties::{Digest, Measured};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(character: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL)
}

fn file(name: &str) -> (VfsPath, String) {
    (VfsPath::new(&format!("/d/{name}")), name.to_string())
}

fn dialog_for(names: &[&str]) -> PropertiesDialog {
    PropertiesDialog::new("Properties".to_string(), names.iter().map(|name| file(name)).collect())
}

fn measured_dialog(names: &[&str]) -> PropertiesDialog {
    let mut dialog = dialog_for(names);
    dialog.set_measured(Ok(Measured { bytes: 10, files: 1, ..Measured::default() }));
    dialog
}

fn digest() -> Digest {
    Digest { hex: ABC.to_string(), bytes: 3 }
}

fn draw(dialog: &PropertiesDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_properties(frame, frame.area(), dialog, &theme)).unwrap();
    terminal
}

fn type_text(dialog: &mut PropertiesDialog, text: &str) {
    for character in text.chars() {
        dialog.handle_key(key(KeyCode::Char(character)));
    }
}

#[test]
fn s_asks_to_compute_the_files_and_is_ignored_while_hashing() {
    let mut dialog = measured_dialog(&["a.iso", "b.iso"]);

    let first = dialog.handle_key(key(KeyCode::Char('s')));
    let second = dialog.handle_key(key(KeyCode::Char('s')));

    assert_eq!(first, PropertiesChoice::Compute(vec![VfsPath::new("/d/a.iso"), VfsPath::new("/d/b.iso")]));
    assert_eq!(second, PropertiesChoice::Pending);
}

#[test]
fn s_may_be_pressed_again_once_every_file_has_an_answer() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));

    assert!(matches!(dialog.handle_key(key(KeyCode::Char('s'))), PropertiesChoice::Compute(_)));
}

#[test]
fn s_does_nothing_without_files() {
    let mut dialog = measured_dialog(&[]);

    assert_eq!(dialog.handle_key(key(KeyCode::Char('s'))), PropertiesChoice::Pending);
}

#[test]
fn esc_cancels_a_running_measure_then_closes() {
    let mut dialog = dialog_for(&["a.iso"]);

    assert_eq!(dialog.handle_key(key(KeyCode::Esc)), PropertiesChoice::CancelRunning);
    assert_eq!(dialog.handle_key(key(KeyCode::Esc)), PropertiesChoice::Close);
}

#[test]
fn esc_cancels_a_running_hash_then_closes_and_q_does_the_same() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));

    assert_eq!(dialog.handle_key(key(KeyCode::Char('q'))), PropertiesChoice::CancelRunning);
    assert_eq!(dialog.handle_key(key(KeyCode::Char('q'))), PropertiesChoice::Close);
}

#[test]
fn a_cancelled_hash_is_drawn_as_cancelled() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.handle_key(key(KeyCode::Esc));

    assert!(find_text(&draw(&dialog, 90, 24), "cancelled").is_some());
}

#[test]
fn esc_closes_at_once_when_nothing_runs() {
    let mut dialog = measured_dialog(&["a.iso"]);

    assert_eq!(dialog.handle_key(key(KeyCode::Esc)), PropertiesChoice::Close);
}

#[test]
fn control_and_alt_chords_are_ignored() {
    let mut dialog = measured_dialog(&["a.iso"]);

    assert_eq!(dialog.handle_key(ctrl('s')), PropertiesChoice::Pending);
    assert_eq!(dialog.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT)), PropertiesChoice::Pending);
}

#[test]
fn the_expected_field_is_unavailable_for_several_files() {
    let mut dialog = measured_dialog(&["a.iso", "b.iso"]);

    dialog.handle_key(key(KeyCode::Char('e')));
    type_text(&mut dialog, "abc");

    assert!(find_text(&draw(&dialog, 90, 24), "Expected").is_none());
}

#[test]
fn typing_into_the_expected_field_never_triggers_a_command() {
    let mut dialog = measured_dialog(&["a.iso"]);

    dialog.handle_key(key(KeyCode::Char('e')));
    for character in "sha256:qe".chars() {
        assert_eq!(dialog.handle_key(key(KeyCode::Char(character))), PropertiesChoice::Pending);
    }
    dialog.handle_key(key(KeyCode::Backspace));

    assert!(find_text(&draw(&dialog, 90, 24), "sha256:q").is_some());
}

#[test]
fn enter_or_esc_leaves_the_expected_field_without_closing() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('e')));

    assert_eq!(dialog.handle_key(key(KeyCode::Esc)), PropertiesChoice::Pending);
    dialog.handle_key(key(KeyCode::Char('e')));
    assert_eq!(dialog.handle_key(key(KeyCode::Enter)), PropertiesChoice::Pending);
    assert_eq!(dialog.handle_key(key(KeyCode::Esc)), PropertiesChoice::Close);
}

fn checked(expected: &str) -> Terminal<TestBackend> {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));
    dialog.handle_key(key(KeyCode::Char('e')));
    type_text(&mut dialog, expected);
    draw(&dialog, 100, 30)
}

#[test]
fn a_matching_expected_digest_says_matches_in_the_success_colour() {
    let theme = indexed();

    let terminal = checked(ABC);

    let (x, y) = find_text(&terminal, "matches").expect("a match line");
    assert_eq!(cell_fg(&terminal, x, y), theme.ok_text().fg.unwrap());
    assert!(find_text(&terminal, "does NOT match").is_none());
}

#[test]
fn a_wrong_expected_digest_says_does_not_match_in_the_error_colour() {
    let theme = indexed();

    let terminal = checked(&"0".repeat(64));

    let (x, y) = find_text(&terminal, "does NOT match").expect("a mismatch line");
    assert_eq!(cell_fg(&terminal, x, y), theme.err_text().fg.unwrap());
}

#[test]
fn a_pasted_sha256sum_line_matches() {
    let terminal = checked(&format!("{}  a.iso", ABC.to_uppercase()));

    assert!(find_text(&terminal, "matches").is_some());
}

#[test]
fn nothing_is_judged_before_there_is_a_digest_or_an_expected_value() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('e')));
    type_text(&mut dialog, ABC);
    let before_digest = draw(&dialog, 100, 30);
    let empty_expected = {
        let mut dialog = measured_dialog(&["a.iso"]);
        dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));
        draw(&dialog, 100, 30)
    };

    for terminal in [before_digest, empty_expected] {
        assert!(find_text(&terminal, "matches").is_none());
        assert!(find_text(&terminal, "does NOT match").is_none());
    }
}

#[test]
fn the_measured_numbers_are_drawn() {
    let mut dialog = dialog_for(&[]);
    dialog.set_measured(Ok(Measured { bytes: 4_521_983_744, files: 1_204, folders: 87, symlinks: 2, unreadable: 1 }));

    let terminal = draw(&dialog, 100, 30);

    assert!(find_text(&terminal, "4.2 GB (4,521,983,744 bytes)").is_some());
    assert!(find_text(&terminal, "1,204 files, 87 folders").is_some());
    assert!(find_text(&terminal, "2 links not followed, 1 folder not readable").is_some());
}

#[test]
fn progress_is_drawn_while_measuring() {
    let mut dialog = dialog_for(&[]);
    dialog.set_progress(Measured { files: 812, folders: 40, ..Measured::default() });

    assert!(find_text(&draw(&dialog, 100, 30), "812 files and 40 folders so far").is_some());
}

#[test]
fn a_failed_measure_shows_the_error_and_no_numbers() {
    let mut dialog = dialog_for(&[]);
    dialog.set_measured(Err("Could not measure the selection:\nConnection reset".into()));

    let terminal = draw(&dialog, 100, 30);

    assert!(find_text(&terminal, "Connection reset").is_some());
    assert!(find_text(&terminal, "files").is_none());
}

#[test]
fn a_typed_measure_failure_is_drawn_as_its_summary_then_its_cause_on_one_line() {
    let mut dialog = dialog_for(&[]);
    dialog
        .set_measured(Err(porthmos_core::Message::with_cause("Could not measure the selection", &"Connection reset")));

    let terminal = draw(&dialog, 100, 30);

    assert!(find_text(&terminal, "Could not measure the selection: Connection reset").is_some());
}

#[test]
fn a_typed_checksum_failure_with_an_escape_sequence_is_drawn_printable() {
    let mut dialog = PropertiesDialog::new("Properties".to_string(), vec![(VfsPath::new("/d/bad"), "bad".to_string())]);
    dialog.set_measured(Ok(Measured::default()));
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(
        &VfsPath::new("/d/bad"),
        Err(porthmos_core::Message::with_cause("Could not read", &"\u{1b}[2Jgone")),
    );

    let terminal = draw(&dialog, 100, 30);

    assert!(!has_unprintable_cell(&terminal));
}

#[test]
fn a_cancelled_measure_is_drawn_as_cancelled_not_as_zero() {
    let mut dialog = dialog_for(&[]);
    dialog.handle_key(key(KeyCode::Esc));

    let terminal = draw(&dialog, 100, 30);

    assert!(find_text(&terminal, "cancelled").is_some());
    assert!(find_text(&terminal, "0 B").is_none());
}

#[test]
fn hostile_names_and_errors_are_printable() {
    let mut dialog = PropertiesDialog::new(
        "Properties".to_string(),
        vec![(VfsPath::new("/d/bad"), "bad\u{1b}[31mname\n\u{202e}gpj.exe".to_string())],
    );
    dialog.set_measured(Ok(Measured::default()));
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(&VfsPath::new("/d/bad"), Err("failed\u{1b}[2J\nnow".into()));

    let terminal = draw(&dialog, 100, 30);

    assert!(!has_unprintable_cell(&terminal));
}

#[test]
fn the_largest_size_is_drawn_without_panicking() {
    let mut dialog = dialog_for(&[]);
    dialog.set_measured(Ok(Measured { bytes: u64::MAX, files: u64::MAX, ..Measured::default() }));

    assert!(find_text(&draw(&dialog, 100, 30), "18,446,744,073,709,551,615 bytes").is_some());
}

#[test]
fn a_wide_terminal_shows_the_digest_on_one_row() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));

    assert!(find_text(&draw(&dialog, 100, 30), ABC).is_some());
}

#[test]
fn a_narrow_terminal_splits_the_digest_without_losing_any_of_it() {
    let rows = digest_rows(ABC, 30);

    assert!(rows.len() > 1);
    assert!(rows.iter().all(|row| row.chars().count() <= 30));
    assert_eq!(rows.concat(), ABC);
    assert_eq!(digest_rows(ABC, 64), vec![ABC.to_string()]);
    assert_eq!(digest_rows(ABC, 0), vec![ABC.to_string()]);
}

#[test]
fn a_file_that_changed_size_while_read_shows_the_bytes_actually_read() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum_progress(&VfsPath::new("/d/a.iso"), 3, 5);
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));

    assert!(find_text(&draw(&dialog, 100, 30), "3 bytes read").is_some());
}

#[test]
fn a_running_hash_shows_how_far_it_is() {
    let mut dialog = measured_dialog(&["a.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum_progress(&VfsPath::new("/d/a.iso"), 2_048, 4_096);

    assert!(find_text(&draw(&dialog, 100, 30), "2.0 KB of 4.0 KB").is_some());
}

#[test]
fn a_failed_hash_shows_its_error_on_that_file_only() {
    let mut dialog = measured_dialog(&["a.iso", "b.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Err("Could not read the file:\nPermission denied".into()));
    dialog.set_checksum(&VfsPath::new("/d/b.iso"), Ok(digest()));

    let terminal = draw(&dialog, 100, 30);

    assert!(find_text(&terminal, "Permission denied").is_some());
    assert!(find_text(&terminal, ABC).is_some());
}

#[test]
fn the_dialog_draws_in_a_tiny_terminal_without_panicking() {
    let mut dialog = measured_dialog(&["a.iso", "b.iso"]);
    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));

    for (width, height) in [(1, 1), (10, 3), (30, 6), (60, 10)] {
        let terminal = draw(&dialog, width, height);
        assert!(screen_rows(&terminal).len() == usize::from(height));
    }
}

fn many_files_dialog(count: usize) -> PropertiesDialog {
    let names: Vec<String> = (0..count).map(|index| format!("f{index:02}.iso")).collect();
    let mut dialog = PropertiesDialog::new(
        "Properties".to_string(),
        names.iter().map(|name| (VfsPath::new(&format!("/d/{name}")), name.clone())).collect(),
    );
    dialog.set_measured(Ok(Measured::default()));
    dialog.handle_key(key(KeyCode::Char('s')));
    for name in &names {
        dialog.set_checksum(&VfsPath::new(&format!("/d/{name}")), Ok(digest()));
    }
    dialog
}

#[test]
fn many_files_can_be_scrolled_to_and_back() {
    let mut dialog = many_files_dialog(40);

    let top = draw(&dialog, 100, 20);
    assert!(find_text(&top, "f00.iso").is_some());
    assert!(find_text(&top, "f39.iso").is_none());

    dialog.handle_key(key(KeyCode::Char('G')));
    let bottom = draw(&dialog, 100, 20);
    assert!(find_text(&bottom, "f39.iso").is_some());
    assert!(find_text(&bottom, "f00.iso").is_none());

    dialog.handle_key(key(KeyCode::Char('g')));
    assert!(find_text(&draw(&dialog, 100, 20), "f00.iso").is_some());
}

#[test]
fn scrolling_by_line_and_page_stops_at_both_ends() {
    let mut dialog = many_files_dialog(40);
    draw(&dialog, 100, 20);

    dialog.handle_key(key(KeyCode::Up));
    dialog.handle_key(key(KeyCode::Char('k')));
    assert!(find_text(&draw(&dialog, 100, 20), "f00.iso").is_some());

    for _ in 0..30 {
        dialog.handle_key(key(KeyCode::PageDown));
    }
    dialog.handle_key(key(KeyCode::Down));
    dialog.handle_key(key(KeyCode::Char('j')));
    assert!(find_text(&draw(&dialog, 100, 20), "f39.iso").is_some());

    dialog.handle_key(key(KeyCode::Char('k')));
    assert!(find_text(&draw(&dialog, 100, 20), "f39.iso").is_some());
    dialog.handle_key(key(KeyCode::PageUp));
    assert!(find_text(&draw(&dialog, 100, 20), "f00.iso").is_none());
}

#[test]
fn the_footer_mentions_scrolling_only_when_there_is_something_to_scroll() {
    let long = many_files_dialog(40);
    let short = measured_dialog(&["a.iso"]);

    assert!(find_text(&draw(&long, 100, 20), "j/k scroll").is_some());
    assert!(find_text(&draw(&short, 100, 20), "scroll").is_none());
}

#[test]
fn a_result_for_a_file_that_is_not_being_hashed_is_ignored() {
    let mut dialog = measured_dialog(&["a.iso"]);

    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));
    assert!(find_text(&draw(&dialog, 100, 30), ABC).is_none());

    dialog.handle_key(key(KeyCode::Char('s')));
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(digest()));
    let other = Digest { hex: "0".repeat(64), bytes: 3 };
    dialog.set_checksum(&VfsPath::new("/d/a.iso"), Ok(other));
    assert!(find_text(&draw(&dialog, 100, 30), ABC).is_some());
}

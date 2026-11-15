use porthmos_core::transfer::Direction;
use porthmos_core::transfer::rows::{QueueRow, RowFailure, RowKind, RowState};

use super::{direction_glyph, fill_width, percent_text, progress, row_hint, row_text};

fn row(state: RowState) -> QueueRow {
    QueueRow {
        kind: RowKind::Single(1),
        label: "photos".to_string(),
        direction: Direction::Upload,
        files_done: 1,
        files_total: 4,
        bytes_done: 25,
        bytes_total: 100,
        state,
        paused: false,
        session_id: Some(1),
        failure: None,
    }
}

#[test]
fn the_fill_is_a_share_of_the_bar_and_never_overflows_it() {
    assert_eq!(fill_width(200.0, 0), 0.0);
    assert_eq!(fill_width(200.0, 50), 100.0);
    assert_eq!(fill_width(200.0, 100), 200.0);
    assert_eq!(fill_width(200.0, 250), 200.0);
}

#[test]
fn a_failed_row_text_carries_the_failure() {
    let mut failed = row(RowState::Failed);
    failed.failure = Some(RowFailure { file: "a.txt".to_string(), message: "denied".to_string(), attempts: 2 });

    assert_eq!(row_text(&failed), "photos  ·  a.txt · 2 attempts · denied");
    assert_eq!(row_text(&row(RowState::Running)), "photos");

    let mut single = row(RowState::Failed);
    single.failure = Some(RowFailure { file: "photos".to_string(), message: "denied".to_string(), attempts: 1 });
    assert_eq!(row_text(&single), "photos  ·  1 attempt · denied");
}

#[test]
fn the_percent_is_blank_while_scanning_and_a_number_otherwise() {
    assert_eq!(percent_text(&row(RowState::Scanning)), "");
    assert_eq!(percent_text(&row(RowState::Running)), "25%");
}

#[test]
fn the_direction_has_a_glyph() {
    assert_eq!(direction_glyph(Direction::Upload), "↑");
    assert_eq!(direction_glyph(Direction::Download), "↓");
}

#[test]
fn only_a_failed_row_has_a_hint_and_it_is_the_whole_text() {
    let mut failed = row(RowState::Failed);
    failed.failure = Some(RowFailure {
        file: "big.bin".to_string(),
        message: "a very long reason that does not fit in the row".to_string(),
        attempts: 3,
    });

    assert_eq!(
        row_hint(&failed).as_deref(),
        Some("photos  ·  big.bin · 3 attempts · a very long reason that does not fit in the row")
    );
    assert_eq!(row_hint(&row(RowState::Running)), None);
}

#[test]
fn a_row_waiting_for_an_answer_or_failed_with_no_bytes_shows_no_progress() {
    let mut waiting = row(RowState::AwaitingAnswer);
    waiting.bytes_done = 0;
    waiting.bytes_total = 0;
    assert_eq!(progress(&waiting), None);
    assert_eq!(percent_text(&waiting), "");

    let mut failed = row(RowState::Failed);
    failed.bytes_done = 0;
    failed.bytes_total = 0;
    assert_eq!(progress(&failed), None);
    assert_eq!(percent_text(&failed), "");
}

#[test]
fn a_finished_empty_transfer_is_complete_and_a_running_one_shows_its_share() {
    let mut empty = row(RowState::Done);
    empty.bytes_done = 0;
    empty.bytes_total = 0;
    assert_eq!(progress(&empty), Some(100));
    assert_eq!(percent_text(&empty), "100%");

    assert_eq!(progress(&row(RowState::Running)), Some(25));
    assert_eq!(progress(&row(RowState::Failed)), Some(25));
}

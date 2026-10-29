use porthmos_core::transfer::Direction;
use porthmos_core::transfer::rows::{QueueRow, RowFailure, RowKind, RowState};

use super::{direction_glyph, fill_width, percent_text, row_text};

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

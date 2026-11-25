use porthmos_term::{GridPoint, Match, SearchDirection};

use super::*;

fn found(start: (i32, u16), end: (i32, u16)) -> Match {
    Match { start: GridPoint { line: start.0, column: start.1 }, end: GridPoint { line: end.0, column: end.1 } }
}

fn view(visible_top: i32, rows: u16, first_line: i32) -> Viewport {
    Viewport { columns: 80, rows, visible_top, first_line }
}

#[test]
fn without_a_previous_match_forward_starts_at_the_visible_top() {
    let point = resume_point(None, SearchDirection::Forward, view(-12, 24, -50));

    assert_eq!(point, GridPoint { line: -12, column: 0 });
}

#[test]
fn without_a_previous_match_backward_starts_at_the_visible_bottom() {
    let point = resume_point(None, SearchDirection::Backward, view(-12, 24, -50));

    assert_eq!(point, GridPoint { line: 11, column: 79 });
}

#[test]
fn forward_continues_one_cell_after_the_previous_match() {
    let previous = found((4, 10), (4, 14));

    assert_eq!(
        resume_point(Some(previous), SearchDirection::Forward, view(0, 24, 0)),
        GridPoint { line: 4, column: 15 }
    );
}

#[test]
fn forward_wraps_to_the_next_line_at_the_last_column() {
    let previous = found((4, 70), (4, 79));

    assert_eq!(
        resume_point(Some(previous), SearchDirection::Forward, view(0, 24, 0)),
        GridPoint { line: 5, column: 0 }
    );
}

#[test]
fn backward_continues_one_cell_before_the_previous_match() {
    let previous = found((4, 10), (4, 14));

    assert_eq!(
        resume_point(Some(previous), SearchDirection::Backward, view(0, 24, 0)),
        GridPoint { line: 4, column: 9 }
    );
}

#[test]
fn backward_wraps_to_the_previous_line_at_column_zero() {
    let previous = found((4, 0), (4, 3));

    assert_eq!(
        resume_point(Some(previous), SearchDirection::Backward, view(0, 24, 0)),
        GridPoint { line: 3, column: 79 }
    );
}

#[test]
fn forward_from_the_very_last_cell_wraps_to_the_first_line_of_the_history() {
    let previous = found((23, 70), (23, 79));

    assert_eq!(
        resume_point(Some(previous), SearchDirection::Forward, view(0, 24, -50)),
        GridPoint { line: -50, column: 0 }
    );
}

#[test]
fn backward_from_the_very_first_cell_wraps_to_the_last_line() {
    let previous = found((-50, 0), (-50, 3));

    assert_eq!(
        resume_point(Some(previous), SearchDirection::Backward, view(0, 24, -50)),
        GridPoint { line: 23, column: 79 }
    );
}

#[test]
fn a_match_on_the_first_screen_line_without_history_wraps_backward_to_the_bottom() {
    let previous = found((0, 0), (0, 1));

    assert_eq!(
        resume_point(Some(previous), SearchDirection::Backward, view(0, 24, 0)),
        GridPoint { line: 23, column: 79 }
    );
}

#[test]
fn degenerate_sizes_do_not_underflow() {
    assert_eq!(
        resume_point(None, SearchDirection::Backward, Viewport { columns: 0, rows: 0, visible_top: 0, first_line: 0 }),
        GridPoint { line: 0, column: 0 }
    );
}

#[test]
fn the_status_appears_only_for_a_missed_non_empty_query() {
    assert_eq!(search_status(false, "abc"), None);
    assert_eq!(search_status(true, ""), None);
    assert_eq!(search_status(true, "abc"), Some("No match".to_string()));
}

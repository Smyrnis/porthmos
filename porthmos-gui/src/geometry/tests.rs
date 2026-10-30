use iced::{Point, Size};

use super::{
    CHROME_HEIGHT, COLUMN_HEADER_HEIGHT, Hit, LIST_TOP, PATH_BAR_HEIGHT, ROW_HEIGHT, STATUS_HEIGHT, TAB_HEIGHT,
    TOOLBAR_HEIGHT, TRANSFER_COLUMN_WIDTH, hit_test, labels_visible, list_viewport, scroll_to_show, visible_range,
};
use crate::app::PanelSide;

#[test]
fn visible_range_covers_the_viewport_plus_overscan() {
    assert_eq!(visible_range(240.0, 240.0, 24.0, 1000, 3), 7..23);
}

#[test]
fn visible_range_at_the_top_starts_at_zero() {
    assert_eq!(visible_range(0.0, 240.0, 24.0, 1000, 3), 0..13);
    assert_eq!(visible_range(-50.0, 240.0, 24.0, 1000, 3), 0..13);
}

#[test]
fn visible_range_is_clamped_to_the_total() {
    assert_eq!(visible_range(1100.0, 240.0, 24.0, 50, 3), 42..50);
    assert_eq!(visible_range(99_999.0, 240.0, 24.0, 50, 3), 50..50);
}

#[test]
fn visible_range_of_an_empty_list_is_empty() {
    assert_eq!(visible_range(0.0, 240.0, 24.0, 0, 3), 0..0);
    assert_eq!(visible_range(500.0, 240.0, 24.0, 0, 3), 0..0);
}

#[test]
fn visible_range_survives_nonsense_geometry() {
    assert_eq!(visible_range(f32::NAN, 240.0, 24.0, 100, 2), 0..12);
    assert_eq!(visible_range(0.0, f32::NAN, 24.0, 100, 2), 0..2);
    assert_eq!(visible_range(0.0, 240.0, 0.0, 100, 2), 0..0);
}

#[test]
fn a_fifty_thousand_row_list_builds_only_the_visible_slice() {
    let range = visible_range(600_000.0, 700.0, 24.0, 50_000, 6);

    assert!(range.len() < 50, "{range:?}");
    assert_eq!(range.start, 24_994);
}

#[test]
fn a_row_already_in_view_keeps_the_offset() {
    assert_eq!(scroll_to_show(5, 0.0, 240.0, 24.0), 0.0);
    assert_eq!(scroll_to_show(9, 0.0, 240.0, 24.0), 0.0);
}

#[test]
fn a_row_below_the_view_scrolls_it_to_the_bottom_edge() {
    assert_eq!(scroll_to_show(10, 0.0, 240.0, 24.0), 24.0);
    assert_eq!(scroll_to_show(100, 0.0, 240.0, 24.0), 2184.0);
}

#[test]
fn a_row_above_the_view_scrolls_it_to_the_top_edge() {
    assert_eq!(scroll_to_show(2, 240.0, 240.0, 24.0), 48.0);
}

#[test]
fn a_view_smaller_than_a_row_still_lands_on_the_row() {
    assert_eq!(scroll_to_show(3, 0.0, 10.0, 24.0), 72.0);
    assert_eq!(scroll_to_show(3, 0.0, f32::NAN, 24.0), 72.0);
}

#[test]
fn the_list_viewport_is_the_window_minus_the_fixed_chrome() {
    assert_eq!(list_viewport(700.0), 700.0 - CHROME_HEIGHT);
    assert_eq!(list_viewport(50.0), ROW_HEIGHT);
}

fn window() -> Size {
    Size::new(1001.0, 700.0)
}

fn hit(x: f32, y: f32) -> Option<Hit> {
    hit_test(window(), Point::new(x, y), (0.0, 0.0), (100, 100), 0.0)
}

#[test]
fn the_chrome_is_the_sum_of_the_fixed_bars() {
    assert_eq!(ROW_HEIGHT, 22.0);
    assert_eq!(LIST_TOP, TAB_HEIGHT + 1.0 + TOOLBAR_HEIGHT + 1.0 + PATH_BAR_HEIGHT + 1.0 + COLUMN_HEADER_HEIGHT + 1.0);
    assert_eq!(LIST_TOP, 32.0 + 1.0 + 44.0 + 1.0 + 36.0 + 1.0 + 24.0 + 1.0);
    assert_eq!(CHROME_HEIGHT, LIST_TOP + 1.0 + STATUS_HEIGHT + 1.0 + STATUS_HEIGHT);
    assert_eq!(CHROME_HEIGHT, 190.0);
}

#[test]
fn labels_are_shown_from_900_pixels_of_width() {
    assert!(!labels_visible(899.9));
    assert!(labels_visible(900.0));
    assert!(!labels_visible(f32::NAN));
}

#[test]
fn hit_test_finds_the_panel_and_row_under_the_pointer_with_scroll() {
    let row_three = LIST_TOP + ROW_HEIGHT * 3.0 + 5.0;

    assert_eq!(hit(100.0, row_three), Some(Hit { side: PanelSide::Local, row: Some(3) }));
    assert_eq!(hit(800.0, row_three), Some(Hit { side: PanelSide::Remote, row: Some(3) }));
    let scrolled = hit_test(window(), Point::new(100.0, row_three), (48.0, 0.0), (100, 100), 0.0);
    assert_eq!(scrolled, Some(Hit { side: PanelSide::Local, row: Some(5) }));
    let remote_scrolled = hit_test(window(), Point::new(800.0, row_three), (48.0, 96.0), (100, 100), 0.0);
    assert_eq!(remote_scrolled, Some(Hit { side: PanelSide::Remote, row: Some(7) }));
}

#[test]
fn hit_test_ignores_the_tab_strip_header_and_status_lines() {
    assert_eq!(hit(100.0, 10.0), None);
    assert_eq!(hit(100.0, TAB_HEIGHT + 5.0), None);
    assert_eq!(hit(100.0, LIST_TOP - 5.0), None);
    let list_bottom = window().height - CHROME_HEIGHT + LIST_TOP;
    assert_eq!(hit(100.0, list_bottom + 3.0), None);
    assert_eq!(hit(100.0, window().height - 10.0), None);
    assert_eq!(hit(100.0, window().height + 50.0), None);
    assert_eq!(hit(-5.0, 200.0), None);
    assert_eq!(hit(window().width + 5.0, 200.0), None);
}

fn left_panel_width() -> f32 {
    (window().width - TRANSFER_COLUMN_WIDTH - 2.0) / 2.0
}

#[test]
fn hit_test_on_the_dividers_hits_nothing() {
    let width = left_panel_width();

    assert_eq!(hit(width + 0.5, 200.0), None);
    assert!(hit(width - 1.0, 200.0).is_some());
    let right_start = width + 1.0 + TRANSFER_COLUMN_WIDTH;
    assert_eq!(hit(right_start + 0.5, 200.0), None);
    assert!(hit(right_start + 1.5, 200.0).is_some());
}

#[test]
fn a_point_over_the_transfer_column_hits_nothing() {
    let width = left_panel_width();

    assert_eq!(hit(width + 1.0 + 20.0, 200.0), None);
}

#[test]
fn the_right_panel_starts_after_the_transfer_column() {
    let width = left_panel_width();

    let right = hit(width + 1.0 + TRANSFER_COLUMN_WIDTH + 1.0 + 5.0, LIST_TOP + 1.0);

    assert_eq!(right, Some(Hit { side: PanelSide::Remote, row: Some(0) }));
}

#[test]
fn rows_are_22_pixels_high() {
    let inside_row_three = LIST_TOP + 22.0 * 3.0 + 5.0;

    assert_eq!(hit(100.0, inside_row_three), Some(Hit { side: PanelSide::Local, row: Some(3) }));
    assert_eq!(hit(100.0, LIST_TOP + 22.0 * 3.0 - 1.0), Some(Hit { side: PanelSide::Local, row: Some(2) }));
}

#[test]
fn hit_test_past_the_last_row_hits_the_panel_only() {
    let point = Point::new(100.0, LIST_TOP + ROW_HEIGHT * 5.0 + 1.0);

    let hit = hit_test(window(), point, (0.0, 0.0), (5, 5), 0.0);

    assert_eq!(hit, Some(Hit { side: PanelSide::Local, row: None }));
    let none = hit_test(window(), Point::new(100.0, LIST_TOP + 1.0), (0.0, 0.0), (0, 0), 0.0);
    assert_eq!(none, Some(Hit { side: PanelSide::Local, row: None }));
}

#[test]
fn an_open_queue_pane_takes_the_bottom_of_the_list() {
    let near_bottom = window().height - CHROME_HEIGHT + LIST_TOP - 10.0;

    let open = hit_test(window(), Point::new(100.0, near_bottom), (0.0, 0.0), (100, 100), 221.0);
    let closed = hit_test(window(), Point::new(100.0, near_bottom), (0.0, 0.0), (100, 100), 0.0);

    assert_eq!(open, None);
    assert!(closed.is_some());
}

#[test]
fn a_nonsense_pointer_hits_nothing() {
    let nan = hit_test(window(), Point::new(f32::NAN, f32::NAN), (0.0, 0.0), (10, 10), 0.0);

    assert_eq!(nan, None);
}

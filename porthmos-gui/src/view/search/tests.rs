use iced::Size;

use crate::geometry::ROW_HEIGHT;
use crate::testing::Harness;
use crate::windows::SEARCH_WINDOW_SIZE;

use super::*;

#[test]
fn the_results_area_leaves_room_for_the_chrome_and_never_shrinks_below_one_row() {
    assert_eq!(results_height(SEARCH_WINDOW_SIZE), SEARCH_WINDOW_SIZE.height - SEARCH_CHROME);
    assert!(results_height(Size::new(480.0, 360.0)) >= ROW_HEIGHT);
    assert_eq!(results_height(Size::new(10.0, 10.0)), ROW_HEIGHT);
}

#[test]
fn the_window_view_exists_only_while_search_is_open() {
    let mut harness = Harness::new();
    assert!(window(&harness.app).is_none());

    harness.press(crate::app::Message::MoreItemChosen(crate::menus::MoreItem::Search));

    assert!(window(&harness.app).is_some());
}

#[test]
fn the_list_height_is_unchanged_when_the_preview_is_off() {
    let window = SEARCH_WINDOW_SIZE;

    assert_eq!(search_list_height(window, false), results_height(window));
    assert_eq!(search_preview_pane_height(window, false), None);
}

#[test]
fn the_pane_takes_room_from_the_list_when_the_preview_is_on() {
    let window = SEARCH_WINDOW_SIZE;

    let pane = search_preview_pane_height(window, true).unwrap();

    assert!(pane >= 4.0 * ROW_HEIGHT);
    assert!(search_list_height(window, true) + pane < results_height(window));
    assert!(search_list_height(window, true) >= 3.0 * ROW_HEIGHT);
}

#[test]
fn a_small_window_keeps_the_whole_list_and_has_no_pane() {
    let window = Size::new(480.0, 300.0);

    assert_eq!(search_preview_pane_height(window, true), None);
    assert_eq!(search_list_height(window, true), results_height(window));
}

#[test]
fn the_gap_between_list_and_preview_pane_holds_a_hairline_between_two_dialog_gaps() {
    assert_eq!(PREVIEW_PANE_GAP, 2.0 * crate::view::spacing::DIALOG_GAP + 1.0);
}

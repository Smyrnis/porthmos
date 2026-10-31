use iced::Size;

use crate::geometry::ROW_HEIGHT;
use crate::testing::Harness;

use super::*;
use crate::bookmarks::BookmarksState;
use crate::tools::Tool;

#[test]
fn the_panel_keeps_its_size_in_a_large_window() {
    assert_eq!(panel_size(Size::new(1100.0, 700.0)), Size::new(TOOL_WIDTH, TOOL_HEIGHT));
}

#[test]
fn the_panel_shrinks_with_a_small_window() {
    assert_eq!(panel_size(Size::new(480.0, 320.0)), Size::new(480.0 - 2.0 * MARGIN, 320.0 - 2.0 * MARGIN));
}

#[test]
fn the_results_area_leaves_room_for_the_chrome_and_never_shrinks_below_one_row() {
    let large = results_height(Size::new(1100.0, 700.0));
    assert_eq!(large, TOOL_HEIGHT - CHROME_HEIGHT);
    assert!(results_height(Size::new(480.0, 320.0)) >= ROW_HEIGHT);
    assert_eq!(results_height(Size::new(10.0, 10.0)), ROW_HEIGHT);
}

#[test]
fn the_overlay_is_absent_when_no_tool_is_open() {
    let harness = Harness::new();

    assert!(overlay(&harness.app).is_none());
}

#[test]
fn the_overlay_is_present_when_search_is_open() {
    let mut harness = Harness::new();
    harness.press(crate::app::Message::MoreItemChosen(crate::menus::MoreItem::Search));

    assert!(overlay(&harness.app).is_some());
}

#[test]
fn the_bookmark_dialog_has_its_own_size() {
    let window = Size::new(1100.0, 700.0);

    assert_eq!(
        frame_size(&Tool::Bookmarks(BookmarksState::new(Vec::new())), window),
        Size::new(BOOKMARKS_WIDTH, BOOKMARKS_HEIGHT)
    );
}

#[test]
fn the_list_area_of_each_tool_never_shrinks_below_one_row() {
    let tiny = Size::new(10.0, 10.0);

    assert_eq!(list_height(&Tool::Bookmarks(BookmarksState::new(Vec::new())), tiny), ROW_HEIGHT);
    assert_eq!(
        list_height(&Tool::Bookmarks(BookmarksState::new(Vec::new())), Size::new(1100.0, 700.0)),
        BOOKMARKS_HEIGHT - BOOKMARKS_CHROME
    );
}

#[test]
fn the_empty_hint_points_at_the_add_action() {
    assert_eq!(bookmarks_hint(0), Some("No bookmarks yet. Use Bookmark this folder."));
    assert_eq!(bookmarks_hint(2), None);
}

#[test]
fn the_history_dialog_has_its_own_size_and_list_height() {
    let window = Size::new(1100.0, 700.0);
    let tool = Tool::History(crate::history_view::HistoryState::new(Vec::new()));

    assert_eq!(frame_size(&tool, window), Size::new(HISTORY_WIDTH, HISTORY_HEIGHT));
    assert_eq!(list_height(&tool, window), HISTORY_HEIGHT - HISTORY_CHROME);
    assert_eq!(list_height(&tool, Size::new(10.0, 10.0)), ROW_HEIGHT);
}

#[test]
fn the_history_hint_depends_on_the_filter() {
    assert_eq!(history_hint(0, 0), Some("No transfers yet."));
    assert_eq!(history_hint(0, 5), Some("No transfers match."));
    assert_eq!(history_hint(3, 5), None);
}

#[test]
fn the_bookmark_place_column_gets_fewer_cells_in_a_narrower_window() {
    let wide = bookmark_place_cells(Size::new(1100.0, 700.0));
    let narrow = bookmark_place_cells(Size::new(480.0, 320.0));

    assert!(wide > narrow);
    assert!(narrow >= 1);
}

#[test]
fn a_long_bookmark_path_is_shortened_with_an_ellipsis_to_the_column() {
    let long = "/home/tester/projects/a/very/deep/folder/structure/that/does/not/fit/in/the/column";

    let shown = fit_place(long, 20);

    assert!(shown.ends_with('\u{2026}'));
    assert!(unicode_width::UnicodeWidthStr::width(shown.as_str()) <= 20);
    assert_eq!(fit_place("/short", 20), "/short");
}

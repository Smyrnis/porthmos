use iced::Size;

use super::*;

#[test]
fn the_panel_keeps_its_size_in_a_large_window() {
    assert_eq!(panel_size(Size::new(1100.0, 700.0)), Size::new(MANAGER_WIDTH, MANAGER_HEIGHT));
}

#[test]
fn the_panel_shrinks_with_a_small_window() {
    let size = panel_size(Size::new(480.0, 320.0));
    assert_eq!(size, Size::new(480.0 - 2.0 * MARGIN, 320.0 - 2.0 * MARGIN));
}

#[test]
fn the_panel_never_has_a_negative_size() {
    let size = panel_size(Size::new(10.0, 10.0));
    assert_eq!(size, Size::new(0.0, 0.0));
}

#[test]
fn group_text_shows_the_count() {
    assert_eq!(group_text("Work", 3), "Work (3)");
}

#[test]
fn the_empty_hint_depends_on_the_filter() {
    assert_eq!(empty_hint(0, false), Some("No saved connections. Click Add to create one."));
    assert_eq!(empty_hint(0, true), Some("No connections match"));
    assert_eq!(empty_hint(2, true), None);
}

#[test]
fn the_details_pane_hides_in_a_narrow_panel() {
    assert!(details_visible(MANAGER_WIDTH));
    assert!(details_visible(DETAILS_MIN_PANEL));
    assert!(!details_visible(DETAILS_MIN_PANEL - 1.0));
}

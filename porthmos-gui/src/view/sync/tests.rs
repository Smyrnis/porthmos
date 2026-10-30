use std::sync::Arc;

use iced::Size;
use porthmos_core::sync::{SyncAction, SyncBy, SyncDirection, SyncFacts, SyncItem, SyncOptions, SyncPlan, SyncReason};
use porthmos_core::{Event, VfsPath};

use super::{
    SYNC_STACK_EXTRA, SYNC_TIMES_MIN_WIDTH, glyph, row_texts, run_enabled, show_times, stacked_buttons, window_for,
};
use crate::geometry::ROW_HEIGHT;
use crate::sync_view::SyncView;
use crate::testing::Harness;
use crate::view::tools::{SYNC_HEIGHT, SYNC_WIDTH, frame_size, overlay, sync_list_height};

fn copy_item(id: u32, path: &str) -> SyncItem {
    SyncItem {
        id,
        path: VfsPath::from(path),
        local: Some(SyncFacts { size: 2_048, modified: Some(86_400) }),
        remote: None,
        action: SyncAction::Upload,
        reason: SyncReason::OnlyLocal,
        ticked: true,
        flippable: false,
    }
}

fn plan(items: Vec<SyncItem>) -> Arc<SyncPlan> {
    Arc::new(SyncPlan {
        sync_id: 11,
        session: 3,
        local_root: VfsPath::from("/home/me/site"),
        remote_root: VfsPath::from("/srv/site"),
        options: SyncOptions { direction: SyncDirection::LocalToRemote, by: SyncBy::Time, subfolders: true },
        items,
        skipped_symlinks: 0,
        unreadable_folders: Vec::new(),
        case_clashes: 0,
        unstorable: 0,
    })
}

fn many(rows: u32) -> Arc<SyncPlan> {
    plan((0..rows).map(|id| copy_item(id, &format!("dir/file-{id:06}.txt"))).collect())
}

#[test]
fn the_overlay_builds_for_a_plan_with_many_rows() {
    let mut harness = Harness::new();
    harness.feed(Event::SyncPlanReady(many(100_000)));

    assert!(overlay(&harness.app).is_some());
}

#[test]
fn the_overlay_builds_for_an_empty_plan_and_a_filter_with_no_matches() {
    let mut harness = Harness::new();
    harness.feed(Event::SyncPlanReady(plan(Vec::new())));
    assert!(overlay(&harness.app).is_some());

    let mut other = Harness::new();
    other.feed(Event::SyncPlanReady(many(10)));
    other.press(crate::app::Message::SyncFilterTyped("zzz".to_string()));
    assert!(overlay(&other.app).is_some());
}

#[test]
fn only_the_visible_window_of_rows_is_built() {
    let viewport = sync_list_height(Size::new(1200.0, 800.0));

    let (first, count) = window_for(0.0, viewport, 100_000);
    assert_eq!(first, 0);
    assert!(count < 100, "built {count} rows");

    let (first, count) = window_for(50_000.0 * ROW_HEIGHT, viewport, 100_000);
    assert!(first > 49_000);
    assert!(count < 100, "built {count} rows");

    assert_eq!(window_for(0.0, viewport, 0), (0, 0));
}

#[test]
fn times_columns_appear_only_in_a_wide_frame() {
    assert!(show_times(SYNC_TIMES_MIN_WIDTH));
    assert!(show_times(SYNC_WIDTH));
    assert!(!show_times(SYNC_TIMES_MIN_WIDTH - 1.0));
    assert!(!show_times(448.0));
}

#[test]
fn the_sync_frame_shrinks_with_the_window_and_keeps_one_row() {
    let large = frame_size(&crate::tools::Tool::Sync, Size::new(1400.0, 900.0));
    assert_eq!((large.width, large.height), (SYNC_WIDTH, SYNC_HEIGHT));

    let small = frame_size(&crate::tools::Tool::Sync, Size::new(480.0, 320.0));
    assert_eq!((small.width, small.height), (448.0, 288.0));

    assert!(sync_list_height(Size::new(480.0, 320.0)) >= ROW_HEIGHT);
    assert_eq!(sync_list_height(Size::new(10.0, 10.0)), ROW_HEIGHT);
}

#[test]
fn run_is_disabled_without_visible_ticks() {
    let mut view = SyncView::default();
    assert!(!run_enabled(&view));

    view.load(many(3));
    assert!(run_enabled(&view));

    view.untick_all();
    assert!(!run_enabled(&view));

    view.tick_all();
    view.set_filter("nothing like it".to_string());
    assert!(!run_enabled(&view));
}

#[test]
fn hostile_paths_are_printable_in_rows() {
    let mut view = SyncView::default();
    view.load(plan(vec![copy_item(0, "bad\u{1b}[31mname\nnext\u{7f}.txt")]));

    let texts = row_texts(&view.row(0).unwrap());

    assert!(!texts.path.chars().any(char::is_control), "{:?}", texts.path);
    assert!(texts.path.starts_with("bad?[31mname?next"));
}

#[test]
fn a_row_shows_the_glyph_the_reason_the_size_and_both_times() {
    let mut view = SyncView::default();
    view.load(plan(vec![copy_item(0, "a.txt")]));

    let texts = row_texts(&view.row(0).unwrap());

    assert_eq!(texts.glyph, "\u{2191}");
    assert_eq!(texts.reason, "new locally");
    assert_eq!(texts.size, "2.0K");
    assert_eq!(texts.remote, "\u{2014}");
    assert!(texts.local.starts_with("1970-01-0"), "{}", texts.local);
}

#[test]
fn the_action_glyphs_are_distinct() {
    assert_eq!(glyph(SyncAction::Upload), "\u{2191}");
    assert_eq!(glyph(SyncAction::Download), "\u{2193}");
    assert_eq!(glyph(SyncAction::Skip), "\u{b7}");
}

#[test]
fn the_buttons_stack_in_a_narrow_frame_and_the_list_gives_up_the_room() {
    assert!(stacked_buttons(448.0));
    assert!(!stacked_buttons(SYNC_WIDTH));

    let narrow = sync_list_height(Size::new(480.0, 800.0));
    let wide = sync_list_height(Size::new(1400.0, 800.0));

    assert_eq!(wide - narrow, SYNC_STACK_EXTRA);
}

use std::sync::Arc;

use chrono::{Local, TimeZone};
use porthmos_core::VfsPath;
use porthmos_core::sync::{SyncAction, SyncBy, SyncDirection, SyncFacts, SyncItem, SyncOptions, SyncPlan, SyncReason};

use super::{SyncView, reason_text, summary_text};

fn facts(size: u64, modified: u64) -> Option<SyncFacts> {
    Some(SyncFacts { size, modified: Some(modified) })
}

fn item(
    id: u32, path: &str, action: SyncAction, reason: SyncReason, ticked: bool, local: Option<SyncFacts>,
    remote: Option<SyncFacts>,
) -> SyncItem {
    let flippable = !ticked
        && matches!(
            reason,
            SyncReason::TargetNewer | SyncReason::SameTimeDifferentSize | SyncReason::SizeDiffersTimeUnknown
        );
    SyncItem { id, path: VfsPath::from(path), local, remote, action, reason, ticked, flippable }
}

fn plan(direction: SyncDirection, items: Vec<SyncItem>) -> Arc<SyncPlan> {
    Arc::new(SyncPlan {
        sync_id: 11,
        session: 3,
        local_root: VfsPath::from("/home/me/site"),
        remote_root: VfsPath::from("/srv/site"),
        options: SyncOptions { direction, by: SyncBy::Time, subfolders: true },
        items,
        skipped_symlinks: 0,
        unreadable_folders: Vec::new(),
        case_clashes: 0,
        unstorable: 0,
    })
}

fn sample() -> SyncView {
    let mut view = SyncView::default();
    view.load(plan(
        SyncDirection::LocalToRemote,
        vec![
            item(0, "a.txt", SyncAction::Upload, SyncReason::OnlyLocal, true, facts(100, 1_000), None),
            item(
                1,
                "b.txt",
                SyncAction::Upload,
                SyncReason::LocalNewer,
                true,
                facts(2_000, 5_000),
                facts(1_900, 1_000),
            ),
            item(2, "c.txt", SyncAction::Skip, SyncReason::TargetNewer, false, facts(10, 1_000), facts(10, 9_000)),
            item(3, "dir", SyncAction::Skip, SyncReason::KindMismatch, false, facts(1, 1), None),
        ],
    ));
    view
}

fn choices(view: &SyncView) -> Vec<(u32, SyncAction)> {
    view.visible_choices()
}

#[test]
fn load_starts_with_the_plans_ticks_and_actions() {
    let view = sample();

    assert!(view.has_plan());
    assert_eq!(view.total(), 4);
    assert_eq!(view.matched(), 4);
    assert_eq!(view.sync_id(), Some(11));
    assert_eq!(view.plan().map(|plan| plan.items.len()), Some(4));
    assert_eq!(choices(&view), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
    assert_eq!(view.cursor(), 0);
    assert_eq!(view.selected_item(), Some(0));
}

#[test]
fn toggle_unticks_and_reticks_a_copy_row() {
    let mut view = sample();

    view.toggle();
    assert_eq!(choices(&view), vec![(1, SyncAction::Upload)]);

    view.toggle();
    assert_eq!(choices(&view), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn toggle_on_a_skipped_flippable_row_flips_and_ticks_it() {
    let mut view = sample();
    view.select(2);

    view.toggle();

    assert_eq!(choices(&view).last(), Some(&(2, SyncAction::Upload)));
}

#[test]
fn toggle_on_a_kind_mismatch_stays_unticked() {
    let mut view = sample();
    view.select(3);

    view.toggle();
    view.toggle();

    assert!(!choices(&view).iter().any(|(id, _)| *id == 3));
}

#[test]
fn flip_cycles_through_the_allowed_actions_only() {
    let mut one_way = SyncView::default();
    one_way.load(plan(
        SyncDirection::LocalToRemote,
        vec![item(0, "odd.txt", SyncAction::Skip, SyncReason::SameTimeDifferentSize, false, facts(1, 5), facts(2, 5))],
    ));
    one_way.flip();
    assert_eq!(choices(&one_way), vec![(0, SyncAction::Upload)]);
    one_way.flip();
    assert!(choices(&one_way).is_empty());
    one_way.flip();
    assert_eq!(choices(&one_way), vec![(0, SyncAction::Upload)]);

    let mut both = SyncView::default();
    both.load(plan(
        SyncDirection::Both,
        vec![item(0, "odd.txt", SyncAction::Skip, SyncReason::SameTimeDifferentSize, false, facts(1, 5), facts(2, 5))],
    ));
    both.flip();
    assert_eq!(choices(&both), vec![(0, SyncAction::Upload)]);
    both.flip();
    assert_eq!(choices(&both), vec![(0, SyncAction::Download)]);
    both.flip();
    assert!(choices(&both).is_empty());
}

#[test]
fn flip_on_an_unflippable_row_does_nothing() {
    let mut view = sample();
    view.select(3);
    assert!(!view.can_flip());

    view.flip();
    view.select(0);
    assert!(!view.can_flip());
    view.flip();

    assert_eq!(choices(&view), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn can_flip_follows_the_selected_row() {
    let mut view = sample();
    assert!(!view.can_flip());

    view.select(2);

    assert!(view.can_flip());
}

#[test]
fn tick_all_and_untick_all_touch_only_visible_rows() {
    let mut view = sample();
    view.set_filter("a.txt".to_string());

    view.untick_all();
    view.set_filter(String::new());
    assert_eq!(choices(&view), vec![(1, SyncAction::Upload)]);

    view.set_filter("b.txt".to_string());
    view.untick_all();
    view.set_filter("a.txt".to_string());
    view.tick_all();
    view.set_filter(String::new());
    assert_eq!(choices(&view), vec![(0, SyncAction::Upload)]);
}

#[test]
fn tick_all_restores_the_default_action_of_each_visible_row() {
    let mut view = SyncView::default();
    view.load(plan(
        SyncDirection::Both,
        vec![item(0, "x.txt", SyncAction::Download, SyncReason::RemoteNewer, true, facts(1, 1), facts(2, 9))],
    ));
    view.untick_all();
    assert!(choices(&view).is_empty());

    view.flip();
    view.flip();
    view.flip();
    view.tick_all();

    assert_eq!(choices(&view), vec![(0, SyncAction::Download)]);
}

#[test]
fn tick_all_never_ticks_a_row_that_defaults_to_skip() {
    let mut view = sample();
    view.untick_all();

    view.tick_all();

    assert_eq!(choices(&view), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn visible_choices_are_ticked_visible_non_skip_rows_with_their_current_action() {
    let mut view = sample();
    view.select(2);
    view.toggle();
    assert_eq!(choices(&view), vec![(0, SyncAction::Upload), (1, SyncAction::Upload), (2, SyncAction::Upload)]);

    view.set_filter("b.txt".to_string());

    assert_eq!(choices(&view), vec![(1, SyncAction::Upload)]);
}

#[test]
fn a_filter_matches_path_substrings_case_insensitively_reason_words_and_globs() {
    let mut view = sample();

    view.set_filter("A.TXT".to_string());
    assert_eq!(view.matched(), 1);

    view.set_filter("newer".to_string());
    assert_eq!(view.matched(), 2);

    view.set_filter("*.txt".to_string());
    assert_eq!(view.matched(), 3);

    view.set_filter("nothing like this".to_string());
    assert_eq!(view.matched(), 0);

    view.set_filter(String::new());
    assert_eq!(view.matched(), 4);
    assert_eq!(view.filter(), None);
}

#[test]
fn the_cursor_stays_on_the_same_item_when_the_filter_changes() {
    let mut view = sample();
    view.select(1);
    assert_eq!(view.selected_item(), Some(1));

    view.set_filter("txt".to_string());
    assert_eq!(view.selected_item(), Some(1));
    assert_eq!(view.cursor(), 1);

    view.set_filter("b.txt".to_string());
    assert_eq!(view.selected_item(), Some(1));
    assert_eq!(view.cursor(), 0);

    view.set_filter(String::new());
    assert_eq!(view.selected_item(), Some(1));
    assert_eq!(view.cursor(), 1);
}

#[test]
fn the_cursor_is_clamped_when_the_filter_empties_the_list() {
    let mut view = sample();
    view.select(3);

    view.set_filter("no such row".to_string());

    assert_eq!(view.matched(), 0);
    assert_eq!(view.cursor(), 0);
    assert_eq!(view.selected_item(), None);
    view.toggle();
    view.flip();
    view.move_cursor(5);
    assert_eq!(view.cursor(), 0);
}

#[test]
fn the_cursor_moves_inside_the_visible_rows() {
    let mut view = sample();

    view.move_cursor(10);
    assert_eq!(view.cursor(), 3);
    view.move_cursor(-10);
    assert_eq!(view.cursor(), 0);
    view.move_cursor(2);
    assert_eq!(view.cursor(), 2);
}

#[test]
fn summary_counts_and_sizes_use_the_current_actions() {
    let mut view = SyncView::default();
    view.load(plan(
        SyncDirection::Both,
        vec![
            item(0, "up.txt", SyncAction::Upload, SyncReason::LocalNewer, true, facts(2_048, 9), facts(1, 1)),
            item(1, "down.txt", SyncAction::Download, SyncReason::RemoteNewer, true, facts(1, 1), facts(4_096, 9)),
            item(2, "skip.txt", SyncAction::Skip, SyncReason::TargetNewer, false, facts(10, 1), facts(10, 9)),
            item(3, "gone.txt", SyncAction::Upload, SyncReason::OnlyLocal, false, facts(7, 1), None),
        ],
    ));

    let summary = view.summary();

    assert_eq!(
        (summary.ticked, summary.uploads, summary.upload_bytes, summary.downloads, summary.download_bytes),
        (2, 1, 2_048, 1, 4_096)
    );
    assert_eq!(summary.skipped, 2);
    assert_eq!(summary_text(&summary), "2 ticked \u{b7} \u{2191} 1 (2.0K) \u{b7} \u{2193} 1 (4.0K) \u{b7} 2 skipped");

    view.select(2);
    view.flip();
    let flipped = view.summary();
    assert_eq!((flipped.ticked, flipped.uploads, flipped.skipped), (3, 2, 1));
}

#[test]
fn top_text_adds_will_run_only_while_filtering() {
    let mut view = sample();
    assert_eq!(view.top_text(), "2 ticked \u{b7} \u{2191} 2 (2.1K) \u{b7} \u{2193} 0 (0) \u{b7} 2 skipped");

    view.set_filter("b.txt".to_string());

    assert_eq!(
        view.top_text(),
        "2 ticked \u{b7} \u{2191} 2 (2.1K) \u{b7} \u{2193} 0 (0) \u{b7} 2 skipped \u{b7} 1 will run"
    );

    view.set_filter(String::new());
    assert!(!view.top_text().contains("will run"));
}

#[test]
fn row_text_makes_hostile_paths_printable_and_shows_dashes_for_missing_times() {
    let mut view = SyncView::default();
    view.load(plan(
        SyncDirection::LocalToRemote,
        vec![
            item(
                0,
                "bad\u{1b}[31mname\nnext.txt",
                SyncAction::Upload,
                SyncReason::OnlyLocal,
                true,
                Some(SyncFacts { size: 5, modified: None }),
                None,
            ),
            item(1, "timed.txt", SyncAction::Upload, SyncReason::LocalNewer, true, facts(9, 86_400), facts(8, 0)),
        ],
    ));

    let hostile = view.row(0).unwrap();
    assert_eq!(hostile.path, "bad?[31mname?next.txt");
    assert_eq!(hostile.local_time, "\u{2014}");
    assert_eq!(hostile.remote_time, "\u{2014}");
    assert_eq!(hostile.size, 5);
    assert_eq!(hostile.reason, "new locally");
    assert!(hostile.ticked);
    assert!(!hostile.flippable);

    let timed = view.row(1).unwrap();
    let expected = Local.timestamp_opt(86_400, 0).single().unwrap().format("%Y-%m-%d %H:%M").to_string();
    assert_eq!(timed.local_time, expected);
    assert_eq!(timed.size, 9);
}

#[test]
fn row_size_follows_the_current_action() {
    let mut view = SyncView::default();
    view.load(plan(
        SyncDirection::Both,
        vec![item(0, "odd.txt", SyncAction::Skip, SyncReason::SameTimeDifferentSize, false, facts(1, 5), facts(2, 5))],
    ));
    assert_eq!(view.row(0).unwrap().size, 1);

    view.flip();
    assert_eq!(view.row(0).unwrap().action, SyncAction::Upload);
    assert_eq!(view.row(0).unwrap().size, 1);

    view.flip();
    assert_eq!(view.row(0).unwrap().action, SyncAction::Download);
    assert_eq!(view.row(0).unwrap().size, 2);
}

#[test]
fn row_positions_follow_the_filter() {
    let mut view = sample();
    view.set_filter("b.txt".to_string());

    assert_eq!(view.row(0).unwrap().path, "b.txt");
    assert!(view.row(1).is_none());
}

#[test]
fn reason_text_matches_the_terminal_front_end() {
    let expected = [
        (SyncReason::OnlyLocal, "new locally"),
        (SyncReason::OnlyRemote, "new on remote"),
        (SyncReason::LocalNewer, "local is newer"),
        (SyncReason::RemoteNewer, "remote is newer"),
        (SyncReason::TargetNewer, "target is newer"),
        (SyncReason::SameTimeDifferentSize, "same time, other size"),
        (SyncReason::SizeDiffers, "size differs"),
        (SyncReason::SizeDiffersTimeUnknown, "size differs, no time"),
        (SyncReason::KindMismatch, "file vs folder"),
    ];

    for (reason, text) in expected {
        assert_eq!(reason_text(reason), text);
    }
}

#[test]
fn clear_forgets_the_plan() {
    let mut view = sample();
    view.set_filter("a".to_string());

    view.clear();

    assert!(!view.has_plan());
    assert_eq!((view.sync_id(), view.total(), view.matched(), view.filter()), (None, 0, 0, None));
    assert!(view.row(0).is_none());
    assert!(view.visible_choices().is_empty());
}

#[test]
fn loading_another_plan_resets_the_filter_and_the_cursor() {
    let mut view = sample();
    view.set_filter("b".to_string());
    view.move_cursor(1);

    view.load(plan(SyncDirection::LocalToRemote, Vec::new()));

    assert_eq!((view.cursor(), view.filter(), view.total()), (0, None, 0));
}

#[test]
fn select_ignores_a_row_past_the_end() {
    let mut view = sample();
    view.select(1);

    view.select(4);
    view.select(100);

    assert_eq!(view.cursor(), 1);
}

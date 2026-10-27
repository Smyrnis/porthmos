use std::sync::Arc;

use porthmos_core::VfsPath;
use porthmos_core::sync::{SyncBy, SyncDirection, SyncFacts, SyncOptions};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

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
    })
}

fn sample() -> SyncView {
    let mut view = SyncView::new();
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

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(view: &SyncView, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_sync(frame, frame.area(), view, &theme)).unwrap();
    terminal
}

fn render(view: &SyncView, width: u16, height: u16) -> String {
    draw(view, width, height).backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn loading_a_plan_takes_the_ticks_and_actions_from_it() {
    let view = sample();

    assert_eq!(view.total(), 4);
    assert_eq!(view.sync_id(), Some(11));
    assert_eq!(view.choices(), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn toggling_unticks_and_reticks_a_normal_row() {
    let mut view = sample();

    view.toggle();
    assert_eq!(view.choices(), vec![(1, SyncAction::Upload)]);

    view.toggle();
    assert_eq!(view.choices(), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn ticking_a_row_that_defaults_to_skip_picks_the_first_runnable_action() {
    let mut view = sample();
    view.move_cursor(2);

    view.toggle();

    assert_eq!(view.choices().last(), Some(&(2, SyncAction::Upload)));
}

#[test]
fn a_file_against_a_folder_cannot_be_ticked_or_flipped() {
    let mut view = sample();
    view.move_cursor(3);

    view.toggle();
    view.flip();

    assert!(!view.choices().iter().any(|(id, _)| *id == 3));
}

#[test]
fn flip_cycles_the_allowed_actions_and_ticks_unless_it_lands_on_skip() {
    let mut view = SyncView::new();
    view.load(plan(
        SyncDirection::Both,
        vec![item(0, "odd.txt", SyncAction::Skip, SyncReason::SameTimeDifferentSize, false, facts(1, 5), facts(2, 5))],
    ));

    view.flip();
    assert_eq!(view.choices(), vec![(0, SyncAction::Upload)]);
    view.flip();
    assert_eq!(view.choices(), vec![(0, SyncAction::Download)]);
    view.flip();
    assert!(view.choices().is_empty());
}

#[test]
fn flip_does_nothing_on_a_row_the_core_does_not_allow_to_change() {
    let mut view = sample();

    view.flip();

    assert_eq!(view.choices(), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn tick_all_never_ticks_a_row_that_defaults_to_skip() {
    let mut view = sample();
    view.untick_all();
    assert!(view.choices().is_empty());

    view.tick_all();

    assert_eq!(view.choices(), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn tick_all_leaves_a_row_that_defaults_to_skip_unticked_on_screen() {
    let mut view = sample();
    view.untick_all();

    view.tick_all();

    assert_eq!(render(&view, 120, 12).matches("[x]").count(), 2);
}

#[test]
fn tick_all_and_untick_all_only_touch_the_rows_the_filter_shows() {
    let mut view = sample();
    view.type_filter('b');
    view.untick_all();
    view.clear_filter();

    assert_eq!(view.choices(), vec![(0, SyncAction::Upload)]);
}

#[test]
fn the_summary_counts_ticked_rows_by_direction_with_their_sizes() {
    let mut view = SyncView::new();
    view.load(plan(
        SyncDirection::Both,
        vec![
            item(0, "up.txt", SyncAction::Upload, SyncReason::OnlyLocal, true, facts(100, 1), None),
            item(1, "down.txt", SyncAction::Download, SyncReason::OnlyRemote, true, None, facts(300, 1)),
            item(2, "down2.txt", SyncAction::Download, SyncReason::RemoteNewer, true, facts(1, 1), facts(700, 9)),
            item(3, "skip.txt", SyncAction::Skip, SyncReason::SameTimeDifferentSize, false, facts(1, 5), facts(2, 5)),
        ],
    ));

    let summary = view.summary();

    assert_eq!(
        (
            summary.ticked,
            summary.uploads,
            summary.upload_bytes,
            summary.downloads,
            summary.download_bytes,
            summary.skipped
        ),
        (3, 1, 100, 2, 1_000, 1)
    );
}

#[test]
fn the_filter_matches_paths_with_text_or_a_glob_and_reasons_by_their_words() {
    let mut view = sample();

    view.type_filter('c');
    assert_eq!(view.matched(), 1);
    view.clear_filter();

    for character in "*.txt".chars() {
        view.type_filter(character);
    }
    assert_eq!(view.matched(), 3);
    view.clear_filter();

    for character in "newer".chars() {
        view.type_filter(character);
    }
    assert_eq!(view.matched(), 2);
}

#[test]
fn the_cursor_stays_inside_the_visible_rows() {
    let mut view = sample();

    view.move_cursor(10);
    assert_eq!(view.cursor(), 3);
    view.move_cursor(-10);
    assert_eq!(view.cursor(), 0);
}

#[test]
fn loading_another_plan_resets_the_filter_and_the_cursor() {
    let mut view = sample();
    view.move_cursor(2);
    view.type_filter('a');

    view.load(plan(SyncDirection::LocalToRemote, Vec::new()));

    assert_eq!((view.cursor(), view.filter(), view.total()), (0, None, 0));
}

#[test]
fn clearing_forgets_the_plan() {
    let mut view = sample();

    view.clear();

    assert!(!view.has_plan() && view.sync_id().is_none() && view.total() == 0);
}

#[test]
fn rows_show_the_tick_the_arrow_the_reason_the_path_and_both_sides() {
    let screen = render(&sample(), 120, 12);

    assert!(screen.contains("[x]"), "{screen}");
    assert!(screen.contains("[ ]"), "{screen}");
    assert!(screen.contains('\u{2191}'), "{screen}");
    assert!(screen.contains("new locally"), "{screen}");
    assert!(screen.contains("target is newer"), "{screen}");
    assert!(screen.contains("file vs folder"), "{screen}");
    assert!(screen.contains("a.txt"), "{screen}");
}

#[test]
fn the_top_bar_summary_counts_ticked_uploads_downloads_and_skipped() {
    let view = sample();

    assert_eq!(view.top_bar_summary(), "2 ticked \u{b7} \u{2191} 2 (2.1K) \u{b7} \u{2193} 0 (0) \u{b7} 2 skipped");
}

#[test]
fn the_filter_line_shows_the_typed_filter_and_the_counts() {
    let mut view = sample();
    view.start_filter();
    view.type_filter('a');

    let terminal = draw(&view, 120, 12);

    assert!(screen_rows(&terminal)[0].starts_with("/ a\u{2588}"));
    assert!(screen_rows(&terminal)[0].trim_end().ends_with("1 of 4"));
}

#[test]
fn a_filter_that_matches_nothing_says_so() {
    let mut view = sample();
    for character in "zzzz".chars() {
        view.type_filter(character);
    }

    assert!(render(&view, 80, 8).contains("no matches"));
}

#[test]
fn control_characters_in_a_path_are_never_drawn_raw() {
    let mut view = SyncView::new();
    view.load(plan(
        SyncDirection::LocalToRemote,
        vec![item(0, "bad\u{1b}[31mname", SyncAction::Upload, SyncReason::OnlyLocal, true, facts(1, 1), None)],
    ));

    let screen = render(&view, 80, 6);

    assert!(!screen.contains('\u{1b}'));
    assert!(screen.contains("bad?[31mname"), "{screen}");
}

#[test]
fn a_very_narrow_terminal_does_not_panic() {
    for width in 0..=12 {
        for height in 0..=4 {
            render(&sample(), width, height);
        }
    }
}

fn grid(view: &SyncView, width: u16, height: u16) -> Vec<Vec<String>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_sync(frame, frame.area(), view, &Theme::default())).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height).map(|y| (0..width).map(|x| buffer[(x, y)].symbol().to_string()).collect()).collect()
}

fn column_of(row: &[String], symbol: &str) -> Option<usize> {
    row.iter().position(|cell| cell == symbol)
}

fn last_text_column(row: &[String]) -> usize {
    row.iter().rposition(|cell| cell.trim() != "").unwrap()
}

fn uneven_paths() -> SyncView {
    let mut view = SyncView::new();
    view.load(plan(
        SyncDirection::LocalToRemote,
        vec![
            item(0, "a.txt", SyncAction::Upload, SyncReason::OnlyLocal, true, facts(100, 1_000), None),
            item(
                1,
                "very/long/directory/name/that/goes/on/and/on/and/on/forever/and/ever/file.txt",
                SyncAction::Upload,
                SyncReason::OnlyLocal,
                true,
                facts(2_000, 5_000),
                None,
            ),
            item(
                2,
                "\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}.txt",
                SyncAction::Upload,
                SyncReason::OnlyLocal,
                true,
                facts(1_234_567, 9_000),
                None,
            ),
        ],
    ));
    view
}

#[test]
fn a_flipped_row_that_defaults_to_skip_is_not_ticked_again_by_tick_all() {
    let mut view = sample();
    view.move_cursor(2);
    view.flip();
    assert_eq!(view.choices().last(), Some(&(2, SyncAction::Upload)));

    view.untick_all();
    view.tick_all();

    assert_eq!(view.choices(), vec![(0, SyncAction::Upload), (1, SyncAction::Upload)]);
}

#[test]
fn tick_all_restores_the_default_action_of_a_row_that_was_flipped() {
    let mut view = SyncView::new();
    let mut flipped = item(0, "x.txt", SyncAction::Upload, SyncReason::LocalNewer, true, facts(5, 9), facts(5, 1));
    flipped.flippable = true;
    view.load(plan(SyncDirection::Both, vec![flipped]));
    view.flip();
    assert_eq!(view.choices(), vec![(0, SyncAction::Download)]);

    view.untick_all();
    view.tick_all();

    assert_eq!(view.choices(), vec![(0, SyncAction::Upload)]);
}

#[test]
fn the_choices_to_run_are_only_the_ticked_rows_the_filter_shows() {
    let mut view = sample();
    view.type_filter('b');

    assert_eq!(view.visible_choices(), vec![(1, SyncAction::Upload)]);

    view.clear_filter();
    assert_eq!(view.visible_choices(), view.choices());
}

#[test]
fn while_filtering_the_top_bar_summary_counts_the_ticked_shown_rows_that_will_run() {
    let mut view = sample();
    view.type_filter('b');

    assert!(view.top_bar_summary().ends_with("\u{b7} 1 will run"), "{}", view.top_bar_summary());
    assert!(!sample().top_bar_summary().contains("will run"));
}

#[test]
fn the_focused_row_stays_focused_when_the_filter_still_shows_it() {
    let mut view = SyncView::new();
    let rows = ["keep1", "drop", "keep2", "drop2", "keep3"]
        .iter()
        .enumerate()
        .map(|(id, name)| item(id as u32, name, SyncAction::Upload, SyncReason::OnlyLocal, true, facts(1, 1), None))
        .collect();
    view.load(plan(SyncDirection::LocalToRemote, rows));
    view.move_cursor(2);

    for character in "keep".chars() {
        view.type_filter(character);
    }
    assert_eq!(view.cursor(), 1);

    view.clear_filter();
    assert_eq!(view.cursor(), 2);
}

#[test]
fn the_cursor_is_clamped_when_the_filter_hides_the_focused_row() {
    let mut view = sample();
    view.move_cursor(3);

    view.type_filter('a');

    assert_eq!(view.cursor(), 0);
}

#[test]
fn long_paths_are_cut_with_an_ellipsis_so_the_columns_stay_aligned() {
    let rows = grid(&uneven_paths(), 120, 8);

    let columns: Vec<_> = (3..=5).map(|y| column_of(&rows[y], "L")).collect();

    assert!(columns[0].is_some(), "{rows:?}");
    assert!(columns.iter().all(|column| *column == columns[0]), "{columns:?}");
    assert!(rows[4].contains(&"\u{2026}".to_string()), "{:?}", rows[4]);
    assert!(rows[5].contains(&"\u{2026}".to_string()), "{:?}", rows[5]);
    assert!(rows[4].concat().contains("2.0K"), "{:?}", rows[4]);
}

#[test]
fn on_a_narrow_screen_the_time_columns_give_way_and_the_size_stays_aligned_and_visible() {
    let rows = grid(&uneven_paths(), 70, 8);

    let ends: Vec<_> = (3..=5).map(|y| last_text_column(&rows[y])).collect();

    assert!(ends.iter().all(|end| *end == ends[0]), "{ends:?}");
    assert!(rows[3].concat().contains("100"), "{:?}", rows[3]);
    assert!(rows[4].concat().contains("2.0K"), "{:?}", rows[4]);
    assert!(rows[5].concat().contains("1.2M"), "{:?}", rows[5]);
}

#[test]
fn bidirectional_and_format_controls_in_a_path_are_never_drawn_raw() {
    let mut view = SyncView::new();
    view.load(plan(
        SyncDirection::LocalToRemote,
        vec![item(
            0,
            "evil\u{202e}txt.exe\u{2066}\u{200f}",
            SyncAction::Upload,
            SyncReason::OnlyLocal,
            true,
            facts(1, 1),
            None,
        )],
    ));

    let screen = render(&view, 100, 6);

    for hidden in ['\u{202e}', '\u{2066}', '\u{200f}'] {
        assert!(!screen.contains(hidden), "{hidden:?}");
    }
    assert!(screen.contains("evil?txt.exe??"), "{screen}");
}

fn row_fg(terminal: &Terminal<TestBackend>, reason: SyncReason) -> Option<ratatui::style::Color> {
    let buffer = terminal.backend().buffer();
    let needle = reason_text(reason);
    (0..buffer.area.height).find_map(|y| {
        let row: String = (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_string()).collect();
        let column = row.find(needle).map(|offset| row[..offset].chars().count() as u16)?;
        buffer[(column, y)].style().fg
    })
}

#[test]
fn rows_take_their_colour_from_the_theme() {
    let theme = Theme::new(ColorDepth::Indexed, &ThemeSettings::default());
    let mut view = sample();
    view.load(plan(
        SyncDirection::LocalToRemote,
        vec![
            item(0, "mismatch", SyncAction::Skip, SyncReason::KindMismatch, false, facts(1, 1), None),
            item(1, "ambiguous", SyncAction::Skip, SyncReason::TargetNewer, false, facts(10, 1), facts(10, 9)),
            item(2, "unticked", SyncAction::Skip, SyncReason::SizeDiffers, false, facts(10, 1), facts(11, 1)),
            item(3, "ticked", SyncAction::Upload, SyncReason::OnlyLocal, true, facts(10, 1), None),
        ],
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 8)).unwrap();
    terminal.draw(|frame| render_sync(frame, frame.area(), &view, &theme)).unwrap();

    assert_eq!(row_fg(&terminal, SyncReason::KindMismatch), Some(theme.err));
    assert_eq!(row_fg(&terminal, SyncReason::TargetNewer), Some(theme.fg_bright));
    assert_eq!(row_fg(&terminal, SyncReason::SizeDiffers), Some(theme.fg_dim));
}

fn row_of(terminal: &Terminal<TestBackend>, needle: &str) -> u16 {
    find_text(terminal, needle).unwrap_or_else(|| panic!("{needle} missing")).1
}

#[test]
fn the_header_row_and_accent_rule_come_after_the_filter_line() {
    let theme = indexed();
    let terminal = draw(&sample(), 120, 12);

    assert!(screen_rows(&terminal)[0].starts_with("/ filter"));
    let (x, y) = find_text(&terminal, "reason").unwrap();
    assert_eq!(y, 1);
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    assert_eq!(terminal.backend().buffer()[(0, 2)].symbol(), "\u{2500}");
    assert_eq!(cell_fg(&terminal, 0, 2), theme.accent);
    assert_eq!(cell_fg(&terminal, 119, 2), theme.accent);
}

#[test]
fn the_header_columns_line_up_with_the_rows() {
    let terminal = draw(&sample(), 120, 12);

    let header_reason = find_text(&terminal, "reason").unwrap().0;
    let row_reason = find_text(&terminal, "new locally").unwrap().0;
    let header_path = find_text(&terminal, "path").unwrap().0;
    let row_path = find_text(&terminal, "a.txt").unwrap().0;
    assert_eq!((header_reason, header_path), (row_reason, row_path));
}

#[test]
fn a_ticked_row_has_an_accent_checkbox_and_an_unticked_one_a_dim_checkbox() {
    let theme = indexed();
    let terminal = draw(&sample(), 120, 12);

    let (x, y) = find_text(&terminal, "[x]").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.accent);
    let (x, y) = find_text(&terminal, "[ ]").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn direction_arrows_are_up_down_and_a_dot_for_skip() {
    let mut view = SyncView::new();
    view.load(plan(
        SyncDirection::Both,
        vec![
            item(0, "up.txt", SyncAction::Upload, SyncReason::OnlyLocal, true, facts(1, 1), None),
            item(1, "down.txt", SyncAction::Download, SyncReason::OnlyRemote, true, None, facts(1, 1)),
            item(2, "skip.txt", SyncAction::Skip, SyncReason::KindMismatch, false, facts(1, 1), None),
        ],
    ));

    let terminal = draw(&view, 120, 12);

    let rows = screen_rows(&terminal);
    assert!(rows[3].contains("[x] \u{2191} "), "{:?}", rows[3]);
    assert!(rows[4].contains("[x] \u{2193} "), "{:?}", rows[4]);
    assert!(rows[5].contains("[ ] \u{b7} "), "{:?}", rows[5]);
}

#[test]
fn times_are_shown_only_when_the_width_allows() {
    let wide = draw(&sample(), 120, 12);
    let narrow = draw(&sample(), 80, 12);

    let wide_header = &screen_rows(&wide)[1];
    let narrow_header = &screen_rows(&narrow)[1];
    assert!(wide_header.contains("local") && wide_header.contains("remote"), "{wide_header:?}");
    assert!(find_text(&wide, "L 19").is_some());
    assert!(!narrow_header.contains("local") && !narrow_header.contains("remote"), "{narrow_header:?}");
    assert!(find_text(&narrow, "L 19").is_none());
}

#[test]
fn the_focused_row_has_a_chevron_and_the_focus_background_and_keeps_its_checkbox_colour() {
    let theme = indexed();
    let mut view = sample();
    view.move_cursor(1);

    let terminal = draw(&view, 120, 12);

    let y = row_of(&terminal, "b.txt");
    assert_eq!(y, 4);
    assert_eq!(terminal.backend().buffer()[(0, y)].symbol(), "\u{203a}");
    assert_eq!(cell_fg(&terminal, 0, y), theme.accent);
    assert_eq!(cell_bg(&terminal, 60, y), theme.row_focus);
    assert_eq!(cell_fg(&terminal, 2, y), theme.accent);
    assert_ne!(cell_bg(&terminal, 60, 3), theme.row_focus);
}

#[test]
fn ticked_rows_use_the_text_colour_for_reason_and_path() {
    let theme = indexed();
    let terminal = draw(&sample(), 120, 12);

    let (x, y) = find_text(&terminal, "new locally").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
    let (x, y) = find_text(&terminal, "a.txt").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
}

#[test]
fn hostile_and_very_long_paths_are_sanitised_and_cut_by_display_width() {
    let mut view = SyncView::new();
    view.load(plan(
        SyncDirection::LocalToRemote,
        vec![item(
            0,
            &"d\u{1b}[2J/\u{202e}\u{65e5}\u{672c}".repeat(80),
            SyncAction::Upload,
            SyncReason::OnlyLocal,
            true,
            facts(1_234_567, 9),
            None,
        )],
    ));

    let terminal = draw(&view, 100, 8);

    assert!(!has_unprintable_cell(&terminal));
    assert!(screen_rows(&terminal)[3].contains("1.2M"));
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier_or_a_box() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
    assert!(!source.contains("Borders"));
}

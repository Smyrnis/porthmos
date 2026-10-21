use chrono::{TimeZone, Utc};
use porthmos_core::transfer::Direction;
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

fn entry(label: &str, connection: &str, result: HistoryResult) -> HistoryEntry {
    HistoryEntry {
        finished_at: Utc.with_ymd_and_hms(2026, 9, 30, 8, 12, 44).unwrap(),
        connection: connection.to_string(),
        direction: Direction::Upload,
        label: label.to_string(),
        local_path: format!("/home/me/{label}"),
        remote_path: format!("/srv/{label}"),
        files_done: 1,
        files_total: 2,
        bytes: 2048,
        result,
        failed_count: 0,
        failed_files: Vec::new(),
    }
}

fn view_of(entries: Vec<HistoryEntry>) -> HistoryView {
    let mut view = HistoryView::new();
    view.replace(entries);
    view
}

fn three() -> HistoryView {
    view_of(vec![
        entry("newest.txt", "prod", HistoryResult::Done),
        entry("middle.txt", "staging", HistoryResult::PartlyFailed { failed: 1 }),
        entry("oldest.txt", "prod", HistoryResult::Failed),
    ])
}

fn labels(view: &HistoryView) -> Vec<&str> {
    view.rows().map(|entry| entry.label.as_str()).collect()
}

fn indexed_theme() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn render(view: &HistoryView, width: u16, height: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_history(frame, frame.area(), view, &indexed_theme())).unwrap();
    terminal
}

fn screen(terminal: &Terminal<TestBackend>) -> String {
    terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
}

#[test]
fn rows_keep_the_order_they_were_given() {
    assert_eq!(labels(&three()), ["newest.txt", "middle.txt", "oldest.txt"]);
}

#[test]
fn the_selection_starts_on_the_first_row_and_moves_within_bounds() {
    let mut view = three();
    assert_eq!(view.selected().unwrap().label, "newest.txt");

    view.move_cursor(1);
    assert_eq!(view.selected().unwrap().label, "middle.txt");
    view.move_cursor(10);
    assert_eq!(view.selected().unwrap().label, "oldest.txt");
    view.move_cursor(-10);
    assert_eq!(view.selected().unwrap().label, "newest.txt");
}

#[test]
fn an_empty_view_selects_nothing_and_ignores_movement() {
    let mut view = HistoryView::new();

    view.move_cursor(1);

    assert!(view.selected().is_none());
    assert_eq!(view.total(), 0);
}

#[test]
fn typing_a_filter_narrows_the_rows_and_erasing_widens_them() {
    let mut view = three();
    view.start_filter();

    for character in "prod".chars() {
        view.type_filter(character);
    }
    assert_eq!(labels(&view), ["newest.txt", "oldest.txt"]);
    assert_eq!((view.matched(), view.total()), (2, 3));

    for _ in 0..4 {
        view.erase_filter();
    }
    assert_eq!(labels(&view).len(), 3);
    assert_eq!(view.filter(), None);
}

#[test]
fn the_word_failed_finds_partly_failed_entries_too() {
    let mut view = three();
    view.start_filter();

    for character in "failed".chars() {
        view.type_filter(character);
    }

    assert_eq!(labels(&view), ["middle.txt", "oldest.txt"]);
}

#[test]
fn a_blank_filter_shows_everything() {
    let mut view = three();
    view.start_filter();

    view.type_filter(' ');

    assert_eq!(labels(&view).len(), 3);
}

#[test]
fn clearing_the_filter_stops_editing_and_shows_everything() {
    let mut view = three();
    view.start_filter();
    view.type_filter('z');
    assert_eq!(labels(&view).len(), 0);

    view.clear_filter();

    assert!(!view.editing_filter());
    assert_eq!(view.filter(), None);
    assert_eq!(labels(&view).len(), 3);
}

#[test]
fn finishing_the_filter_keeps_it_applied() {
    let mut view = three();
    view.start_filter();
    for character in "mid".chars() {
        view.type_filter(character);
    }
    view.finish_filter();

    assert!(!view.editing_filter());
    assert_eq!(view.filter(), Some("mid"));
    assert_eq!(labels(&view), ["middle.txt"]);
}

#[test]
fn replacing_the_entries_keeps_the_filter_and_a_valid_cursor() {
    let mut view = three();
    view.start_filter();
    view.type_filter('t');
    view.finish_filter();
    view.move_cursor(2);
    assert_eq!(view.cursor(), 2);

    view.replace(vec![entry("only.txt", "prod", HistoryResult::Done)]);

    assert_eq!(view.selected().unwrap().label, "only.txt");
    assert_eq!(view.cursor(), 0);
    assert_eq!(view.filter(), Some("t"));
}

#[test]
fn replacing_with_nothing_leaves_no_selection() {
    let mut view = three();
    view.move_cursor(2);

    view.replace(Vec::new());

    assert!(view.selected().is_none());
}

#[test]
fn go_to_top_selects_the_first_row() {
    let mut view = three();
    view.move_cursor(2);

    view.go_to_top();

    assert_eq!(view.cursor(), 0);
}

#[test]
fn the_header_row_is_dim_and_an_accent_rule_follows() {
    let theme = indexed_theme();
    let terminal = render(&three(), 110, 10);

    let (x, y) = find_text(&terminal, "item").unwrap();
    assert_eq!(y, 1);
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    assert_eq!(terminal.backend().buffer()[(0, 2)].symbol(), "\u{2500}");
    assert_eq!(cell_fg(&terminal, 0, 2), theme.accent);
    assert_eq!(cell_fg(&terminal, 109, 2), theme.accent);
}

#[test]
fn the_filter_line_sits_above_the_header() {
    let terminal = render(&three(), 110, 10);

    assert!(screen_rows(&terminal)[0].starts_with("/ filter"));
    assert!(screen_rows(&terminal)[0].trim_end().ends_with('3'));
    assert!(screen_rows(&terminal)[1].contains("time"));
}

#[test]
fn a_row_shows_time_direction_item_connection_files_size_and_status() {
    let terminal = render(&three(), 110, 10);

    let row = screen_rows(&terminal)[3].clone();
    assert!(row.contains("2026-09-30"), "{row:?}");
    assert!(row.contains("\u{2191} newest.txt"), "{row:?}");
    assert!(row.contains("prod"), "{row:?}");
    assert!(row.contains("1/2"), "{row:?}");
    assert!(row.contains("2.0K"), "{row:?}");
    assert!(row.trim_end().ends_with("done"), "{row:?}");
}

#[test]
fn the_focused_row_has_a_chevron_the_focus_background_and_a_bold_name() {
    let theme = indexed_theme();
    let mut view = three();
    view.move_cursor(1);
    let terminal = render(&view, 110, 10);

    let (name_x, y) = find_text(&terminal, "middle.txt").unwrap();
    assert_eq!(y, 4);
    assert_eq!(terminal.backend().buffer()[(0, y)].symbol(), "\u{203a}");
    assert_eq!(cell_fg(&terminal, 0, y), theme.accent);
    assert_eq!(cell_bg(&terminal, 20, y), theme.row_focus);
    assert!(terminal.backend().buffer()[(name_x, y)].modifier.contains(ratatui::style::Modifier::BOLD));
    assert_ne!(cell_bg(&terminal, 20, 3), theme.row_focus);
}

#[test]
fn status_words_and_colours() {
    let theme = indexed_theme();
    let view = view_of(vec![
        entry("a", "prod", HistoryResult::Done),
        entry("b", "prod", HistoryResult::PartlyFailed { failed: 2 }),
        entry("c", "prod", HistoryResult::Failed),
        entry("d", "prod", HistoryResult::Cancelled),
        entry("e", "prod", HistoryResult::Interrupted),
    ]);
    let terminal = render(&view, 110, 12);

    let (status_x, _) = find_text(&terminal, "status").unwrap();
    let rows = screen_rows(&terminal);
    let expected = [
        (3, "done", theme.ok),
        (4, "done, 2 failed", theme.err),
        (5, "failed", theme.err),
        (6, "cancelled", theme.fg_dim),
        (7, "interrupted", theme.fg_dim),
    ];
    for (row, word, color) in expected {
        assert!(rows[row].trim_end().ends_with(word), "{:?}", rows[row]);
        assert_eq!(cell_fg(&terminal, status_x, row as u16), color, "{word}");
    }
}

#[test]
fn columns_are_dropped_in_the_order_size_files_connection_time() {
    let present = |width: u16| {
        let header = screen_rows(&render(&three(), width, 8))[1].clone();
        ["size", "files", "connection", "time", "status"].map(|word| header.contains(word))
    };
    let [size, files, connection, time, status] = present(120);
    assert!(size && files && connection && time && status);

    for width in 14..=120 {
        let [size, files, connection, time, _] = present(width);
        assert!(!size || files, "width {width}: size without files");
        assert!(!files || connection, "width {width}: files without connection");
        assert!(!connection || time, "width {width}: connection without time");
    }
    let [size, files, ..] = present(40);
    assert!(!size && !files);
}

#[test]
fn the_item_column_gets_the_remaining_width() {
    let label = "a-rather-long-transfer-label-that-needs-room.tar.gz";
    let view = view_of(vec![entry(label, "prod", HistoryResult::Done)]);

    assert!(find_text(&render(&view, 140, 8), label).is_some());
    let narrow = render(&view, 70, 8);
    assert!(find_text(&narrow, label).is_none());
    assert!(find_text(&narrow, "\u{2026}").is_some());
}

#[test]
fn an_empty_history_says_no_transfers_yet_in_dim() {
    let theme = indexed_theme();
    let terminal = render(&HistoryView::new(), 60, 8);

    let (x, y) = find_text(&terminal, "no transfers yet").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    assert!(screen_rows(&terminal)[0].starts_with("/ filter"));
}

#[test]
fn a_filter_with_no_match_says_so_and_keeps_the_filter_line() {
    let mut view = three();
    view.start_filter();
    view.type_filter('z');

    let terminal = render(&view, 60, 8);

    assert!(find_text(&terminal, "no matches").is_some());
    assert!(screen_rows(&terminal)[0].starts_with("/ z\u{2588}"));
    assert!(screen_rows(&terminal)[0].trim_end().ends_with("0 of 3"));
}

#[test]
fn the_cursor_row_scrolls_into_view() {
    let entries: Vec<HistoryEntry> =
        (0..40).map(|index| entry(&format!("item{index:02}"), "prod", HistoryResult::Done)).collect();
    let mut view = view_of(entries);
    view.move_cursor(35);

    let terminal = render(&view, 110, 12);

    assert!(find_text(&terminal, "item35").is_some());
    assert!(find_text(&terminal, "item00").is_none());
}

#[test]
fn hostile_labels_and_connection_names_are_sanitised_and_cut() {
    let view = view_of(vec![entry(&"x\u{1b}[2J\u{202e}".repeat(100), &"c\u{7}n".repeat(50), HistoryResult::Done)]);

    let terminal = render(&view, 80, 8);

    assert!(!has_unprintable_cell(&terminal));
    assert!(screen_rows(&terminal)[3].trim_end().ends_with("done"));
}

#[test]
fn a_wide_character_label_stays_inside_its_column() {
    let view = view_of(vec![entry(&"\u{65e5}\u{672c}".repeat(80), "prod", HistoryResult::Done)]);

    let terminal = render(&view, 90, 8);

    assert!(screen_rows(&terminal)[3].trim_end().ends_with("done"));
}

#[test]
fn tiny_screens_do_not_panic() {
    for width in 0..=12 {
        for height in 0..=4 {
            render(&three(), width, height);
            render(&HistoryView::new(), width, height);
        }
    }
}

#[test]
fn the_summary_counts_entries_and_matches() {
    let mut view = three();
    assert_eq!(view.summary(), "3 entries");
    assert_eq!(view_of(vec![entry("a", "prod", HistoryResult::Done)]).summary(), "1 entry");
    assert_eq!(HistoryView::new().summary(), "0 entries");

    view.start_filter();
    view.type_filter('o');
    view.type_filter('l');
    assert_eq!(view.summary(), "1 of 3");
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier_or_a_box() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
    assert!(!source.contains("Borders"));
}

#[test]
fn a_narrow_screen_does_not_panic() {
    for width in [4, 10, 24] {
        render(&three(), width, 6);
    }
}

#[test]
fn a_moderately_narrow_screen_still_shows_the_label() {
    let content = screen(&render(&three(), 40, 8));

    assert!(content.contains("newest"), "{content}");
}

#[test]
fn a_new_entry_arriving_keeps_the_selection_on_the_same_transfer() {
    let mut view = three();
    view.move_cursor(1);
    assert_eq!(view.selected().unwrap().label, "middle.txt");

    view.replace(vec![
        entry("brand-new.txt", "prod", HistoryResult::Done),
        entry("newest.txt", "prod", HistoryResult::Done),
        entry("middle.txt", "staging", HistoryResult::PartlyFailed { failed: 1 }),
        entry("oldest.txt", "prod", HistoryResult::Failed),
    ]);

    assert_eq!(view.selected().unwrap().label, "middle.txt");
    assert_eq!(view.cursor(), 2);
}

#[test]
fn a_label_with_a_newline_stays_on_one_row() {
    let view = view_of(vec![entry("a\nb", "prod", HistoryResult::Done)]);
    let terminal = render(&view, 80, 6);
    let buffer = terminal.backend().buffer();
    let rows: Vec<String> =
        (0..6).map(|y| (0..80).map(|x| buffer[(x, y)].symbol().to_string()).collect::<String>()).collect();

    assert_eq!(rows.iter().filter(|row| row.contains("a?b")).count(), 1, "{rows:?}");
}

#[test]
fn printable_replaces_control_characters_only() {
    assert_eq!(printable("a\nb\u{1b}[0m\tc \u{fc}"), "a?b?[0m?c \u{fc}");
}

#[test]
fn printable_also_replaces_bidirectional_and_format_controls() {
    let hidden = "a\u{202a}\u{202e}b\u{2066}\u{2069}c\u{200e}\u{200f}d\u{61c}e";

    assert_eq!(printable(hidden), "a??b??c??d?e");
}

#[test]
fn a_label_with_a_right_to_left_override_is_never_drawn_raw() {
    let view = view_of(vec![entry("fake\u{202e}txt.exe", "prod", HistoryResult::Done)]);
    let terminal = render(&view, 80, 6);

    let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();

    assert!(!screen.contains('\u{202e}'));
    assert!(screen.contains("fake?txt.exe"), "{screen}");
}

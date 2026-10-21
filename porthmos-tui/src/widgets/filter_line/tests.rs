use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(state: FilterLineState<'_>, width: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal.draw(|frame| render_filter_line(frame, frame.area(), state, &theme)).unwrap();
    terminal
}

fn state(text: Option<&str>, editing: bool, matched: usize, total: usize) -> FilterLineState<'_> {
    FilterLineState { text, editing, matched, total }
}

#[test]
fn nothing_is_shown_without_a_filter_or_the_line() {
    assert_eq!(status(false, None, 3, 3, 40), None);
}

#[test]
fn the_line_shows_the_caret_and_the_count() {
    assert_eq!(status(true, Some("po"), 2, 4, 40).as_deref(), Some("/po\u{2588} (2 of 4)"));
    assert_eq!(status(true, None, 4, 4, 40).as_deref(), Some("/\u{2588} (4 of 4)"));
}

#[test]
fn a_kept_filter_is_labelled() {
    assert_eq!(status(false, Some("po"), 2, 4, 40).as_deref(), Some("filter: po (2 of 4)"));
}

#[test]
fn long_text_keeps_its_end_within_the_width() {
    let shown = status(true, Some("a_really_long_filter_text"), 0, 4, 22).unwrap();
    assert_eq!(shown.chars().count(), 22);
    assert!(shown.starts_with("/\u{2026}") && shown.ends_with("_text\u{2588} (0 of 4)"), "{shown}");
}

#[test]
fn wide_text_is_budgeted_by_display_width_and_keeps_the_caret_and_count() {
    use unicode_width::UnicodeWidthStr;

    for width in 14..40 {
        let shown = status(true, Some("\u{65e5}\u{672c}\u{8a9e}\u{30d5}\u{30a1}\u{30a4}\u{30eb}\u{540d}"), 0, 4, width)
            .unwrap();
        assert!(shown.width() <= width, "width {width}: {shown:?}");
        assert!(shown.ends_with("\u{2588} (0 of 4)"), "width {width}: {shown:?}");
    }
}

#[test]
fn an_idle_line_shows_an_accent_slash_and_the_dim_placeholder() {
    let theme = indexed();
    let terminal = draw(state(None, false, 12, 12), 40);

    assert!(screen_rows(&terminal)[0].starts_with("/ filter"));
    assert_eq!(cell_fg(&terminal, 0, 0), theme.accent);
    assert_eq!(cell_fg(&terminal, 2, 0), theme.fg_dim);
}

#[test]
fn typing_shows_the_bright_text_and_a_block_caret() {
    let theme = indexed();
    let terminal = draw(state(Some("pro"), true, 3, 12), 40);

    assert!(screen_rows(&terminal)[0].starts_with("/ pro\u{2588}"));
    assert_eq!(cell_fg(&terminal, 2, 0), theme.fg_bright);
}

#[test]
fn an_empty_editing_line_shows_only_the_caret() {
    let terminal = draw(state(None, true, 12, 12), 40);

    assert!(screen_rows(&terminal)[0].starts_with("/ \u{2588}"));
}

#[test]
fn a_kept_filter_is_shown_without_a_caret() {
    let terminal = draw(state(Some("pro"), false, 3, 12), 40);

    assert!(screen_rows(&terminal)[0].starts_with("/ pro "));
    assert!(find_text(&terminal, "\u{2588}").is_none());
}

#[test]
fn the_count_is_right_aligned_and_reads_n_of_m_only_while_filtering() {
    let theme = indexed();
    let idle = draw(state(None, false, 12, 12), 40);
    let filtering = draw(state(Some("p"), false, 3, 12), 40);

    assert!(screen_rows(&idle)[0].trim_end().ends_with("12"));
    assert!(!screen_rows(&idle)[0].contains(" of "));
    assert!(screen_rows(&filtering)[0].ends_with("3 of 12"));
    assert_eq!(cell_fg(&filtering, 39, 0), theme.fg_dim);
}

#[test]
fn long_text_keeps_its_end_and_the_count_when_narrow() {
    let terminal = draw(state(Some("a_really_long_filter_text"), true, 3, 12), 30);

    let row = &screen_rows(&terminal)[0];
    assert!(row.contains("\u{2026}"), "{row:?}");
    assert!(row.contains("_text\u{2588}"), "{row:?}");
    assert!(row.ends_with("3 of 12"), "{row:?}");
}

#[test]
fn hostile_text_is_sanitised() {
    let terminal = draw(state(Some("a\u{1b}[2Jb\u{202e}c"), false, 0, 3), 40);

    assert!(!has_unprintable_cell(&terminal));
    assert!(find_text(&terminal, "a?[2Jb?c").is_some());
}

#[test]
fn tiny_widths_do_not_panic() {
    for width in 0..=12 {
        draw(state(Some("abc"), true, 1, 2), width);
        draw(state(None, false, 1, 2), width);
    }
}

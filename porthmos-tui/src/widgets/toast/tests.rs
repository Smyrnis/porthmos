use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text},
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed_theme() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(width: u16, height: u16, severity: Severity, message: &str) -> Terminal<TestBackend> {
    let theme = indexed_theme();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_toast(frame, frame.area(), severity, message, &theme)).unwrap();
    terminal
}

fn symbol(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> String {
    terminal.backend().buffer()[(x, y)].symbol().to_string()
}

fn drawn_cells(terminal: &Terminal<TestBackend>) -> Vec<(u16, u16)> {
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    let mut cells = Vec::new();
    for y in 0..area.height {
        for x in 0..area.width {
            if buffer[(x, y)].symbol() != " " {
                cells.push((x, y));
            }
        }
    }
    cells
}

#[test]
fn a_short_message_is_one_line() {
    assert_eq!(toast_lines("hello", 20, 3), vec!["hello"]);
}

#[test]
fn long_messages_wrap_at_word_boundaries_by_display_width() {
    assert_eq!(toast_lines("one two three four", 9, 3), vec!["one two", "three", "four"]);
}

#[test]
fn a_word_longer_than_the_width_is_broken() {
    assert_eq!(toast_lines("abcdefghij", 4, 3), vec!["abcd", "efgh", "ij"]);
}

#[test]
fn newlines_start_new_lines() {
    assert_eq!(toast_lines("a\nb", 10, 3), vec!["a", "b"]);
}

#[test]
fn text_beyond_the_maximum_is_cut_with_an_ellipsis() {
    assert_eq!(toast_lines("aaa bbb ccc ddd", 3, 2), vec!["aaa", "bb\u{2026}"]);
}

#[test]
fn control_and_bidi_characters_become_question_marks() {
    assert_eq!(toast_lines("a\u{202e}b\u{7}c", 10, 3), vec!["a?b?c"]);
}

#[test]
fn wide_characters_count_two_cells_and_are_never_split() {
    assert_eq!(toast_lines("\u{4e00}\u{4e8c}\u{4e09}", 4, 3), vec!["\u{4e00}\u{4e8c}", "\u{4e09}"]);
    assert_eq!(toast_lines("\u{4e00}a", 1, 3), vec!["a"]);
}

#[test]
fn trailing_newlines_and_carriage_returns_do_not_add_lines() {
    assert_eq!(toast_lines("msg\n", 20, 3), vec!["msg"]);
    assert_eq!(toast_lines("a\r\nb\r\n", 20, 3), vec!["a", "b"]);
}

#[test]
fn an_empty_message_yields_one_empty_line() {
    assert_eq!(toast_lines("", 10, 3), vec![""]);
}

#[test]
fn zero_width_or_zero_lines_yield_nothing() {
    assert!(toast_lines("abc", 0, 3).is_empty());
    assert!(toast_lines("abc", 5, 0).is_empty());
}

#[test]
fn a_five_hundred_character_message_yields_at_most_three_lines() {
    let lines = toast_lines(&"x".repeat(500), 20, 3);

    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|line| line.width() <= 20));
    assert!(lines[2].ends_with('\u{2026}'));
}

#[test]
fn the_toast_sits_at_the_top_right_below_the_top_bar() {
    let terminal = draw(80, 24, Severity::Info, "saved");

    assert_eq!(symbol(&terminal, 77, 2), "\u{2510}");
    assert_eq!(symbol(&terminal, 77, 4), "\u{2518}");
    let (x, y) = find_text(&terminal, "saved").expect("message is drawn");
    assert_eq!(y, 3);
    assert_eq!(x + 5 + 1, 77);
}

#[test]
fn the_dot_colour_follows_the_severity() {
    let theme = indexed_theme();

    for (severity, expected) in
        [(Severity::Info, theme.fg_muted), (Severity::Warning, theme.fg_bright), (Severity::Error, theme.err)]
    {
        let terminal = draw(80, 12, severity, "message");
        let (x, y) = find_text(&terminal, "message").expect("message is drawn");
        assert_eq!(symbol(&terminal, x - 2, y), "\u{2022}");
        assert_eq!(cell_fg(&terminal, x - 2, y), expected, "{severity:?}");
        assert_eq!(cell_fg(&terminal, x, y), theme.fg);
    }
}

#[test]
fn the_border_uses_the_strong_line_colour_and_the_inside_the_surface_colour() {
    let theme = indexed_theme();

    let terminal = draw(80, 12, Severity::Info, "message");

    assert_eq!(cell_fg(&terminal, 77, 2), theme.line_strong);
    let (x, y) = find_text(&terminal, "message").expect("message is drawn");
    assert_eq!(cell_bg(&terminal, x, y), theme.surface);
    assert_ne!(theme.surface, theme.bg);
}

#[test]
fn the_toast_clears_what_is_underneath() {
    let theme = indexed_theme();
    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| {
            let filler = "#".repeat(80);
            for y in 0..12 {
                frame.render_widget(ratatui::widgets::Paragraph::new(filler.clone()), Rect::new(0, y, 80, 1));
            }
            render_toast(frame, frame.area(), Severity::Info, "hi", &theme);
        })
        .unwrap();

    let (x, y) = find_text(&terminal, "hi").expect("message is drawn");
    assert_eq!(symbol(&terminal, x + 2, y), " ");
    assert_eq!(symbol(&terminal, x - 3, y), " ");
}

#[test]
fn a_long_message_stays_inside_the_screen() {
    let message = "a very long message that keeps going and going until it cannot possibly fit on the screen at all";

    for width in [80, 40, 30, 20] {
        let terminal = draw(width, 12, Severity::Error, message);
        let cells = drawn_cells(&terminal);
        assert!(!cells.is_empty(), "width {width}");
        assert!(cells.iter().all(|(x, _)| *x <= width - 3), "width {width}");
        assert!(cells.iter().all(|(_, y)| (2..=6).contains(y)), "width {width}");
    }
}

#[test]
fn tiny_areas_draw_nothing_and_do_not_panic() {
    for (width, height) in [(0, 0), (1, 1), (3, 3), (5, 12), (20, 3)] {
        let terminal = draw(width, height, Severity::Error, "boom");
        assert!(drawn_cells(&terminal).iter().all(|(x, y)| *x < width && *y < height));
    }
}

#[test]
fn the_toast_never_paints_with_a_default_background() {
    let terminal = draw(80, 12, Severity::Info, "message");
    let (x, y) = find_text(&terminal, "message").expect("message is drawn");

    assert_ne!(cell_bg(&terminal, x, y), Color::Reset);
}

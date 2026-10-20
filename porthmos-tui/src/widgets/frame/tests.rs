use ratatui::{
    Frame, Terminal,
    backend::TestBackend,
    layout::Rect,
    style::Color,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg},
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed_theme() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(width: u16, height: u16, render: impl FnOnce(&mut Frame, Rect)) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render(frame, area);
        })
        .unwrap();
    terminal
}

fn row_text(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    let mut x = 0;
    while x < buffer.area.width {
        let symbol = buffer[(x, y)].symbol();
        text.push_str(symbol);
        x += symbol.width().max(1) as u16;
    }
    text
}

fn hint(key: &str, label: &'static str) -> Hint {
    Hint { key: key.to_string(), label }
}

fn sample_hints() -> Hints {
    Hints { entries: vec![hint("F5", "copy"), hint("F7", "mkdir")], pinned: Some(hint("F1", "help")), right: None }
}

fn hint_row(width: u16, hints: &Hints) -> String {
    let theme = indexed_theme();
    let terminal = draw(width, 1, |frame, area| hint_bar(frame, area, hints, &theme));
    row_text(&terminal, 0)
}

#[test]
fn fill_background_paints_every_cell_with_the_theme_bg_and_fg() {
    let theme = indexed_theme();

    let terminal = draw(10, 3, |frame, area| fill_background(frame, area, &theme));

    for y in 0..3 {
        for x in 0..10 {
            assert_eq!(cell_bg(&terminal, x, y), theme.bg);
            assert_eq!(cell_fg(&terminal, x, y), theme.fg);
        }
    }
}

#[test]
fn fill_background_keeps_the_terminal_background_when_the_theme_uses_it() {
    let theme =
        Theme::new(ColorDepth::Indexed, &ThemeSettings { terminal_background: true, ..ThemeSettings::default() });

    let terminal = draw(10, 3, |frame, area| fill_background(frame, area, &theme));

    for y in 0..3 {
        for x in 0..10 {
            assert_eq!(cell_bg(&terminal, x, y), Color::Reset);
        }
    }
}

#[test]
fn clear_popup_erases_text_underneath_and_paints_the_background() {
    let theme = indexed_theme();

    let terminal = draw(10, 1, |frame, area| {
        frame.render_widget(Paragraph::new("hello"), area);
        clear_popup(frame, area, &theme);
    });

    assert_eq!(row_text(&terminal, 0).trim(), "");
    assert_eq!(cell_bg(&terminal, 2, 0), theme.bg);
}

#[test]
fn hairline_draws_a_row_of_light_horizontal_lines_in_the_hairline_colour() {
    let theme = indexed_theme();

    let terminal = draw(12, 2, |frame, area| hairline(frame, Rect::new(0, 0, area.width, 1), &theme));

    assert_eq!(row_text(&terminal, 0), "─".repeat(12));
    assert_eq!(row_text(&terminal, 1).trim(), "");
    for x in 0..12 {
        assert_eq!(cell_fg(&terminal, x, 0), theme.line);
    }
}

#[test]
fn hairline_in_an_empty_area_draws_nothing() {
    let theme = indexed_theme();

    let terminal = draw(5, 1, |frame, _| {
        hairline(frame, Rect::new(0, 0, 0, 1), &theme);
        hairline(frame, Rect::new(0, 0, 5, 0), &theme);
    });

    assert_eq!(row_text(&terminal, 0).trim(), "");
}

#[test]
fn accent_underline_recolours_only_the_requested_columns() {
    let theme = indexed_theme();

    let terminal = draw(20, 1, |frame, area| {
        hairline(frame, area, &theme);
        accent_underline(frame, area, 3..8, &theme);
    });

    for x in 3..8 {
        assert_eq!(cell_fg(&terminal, x, 0), theme.accent);
    }
    assert_eq!(cell_fg(&terminal, 2, 0), theme.line);
    assert_eq!(cell_fg(&terminal, 8, 0), theme.line);
    assert_eq!(row_text(&terminal, 0), "─".repeat(20));
}

#[test]
fn accent_underline_is_clipped_to_the_rule() {
    let theme = indexed_theme();

    let terminal = draw(10, 1, |frame, area| {
        hairline(frame, area, &theme);
        accent_underline(frame, area, 8..40, &theme);
    });

    assert_eq!(cell_fg(&terminal, 7, 0), theme.line);
    assert_eq!(cell_fg(&terminal, 8, 0), theme.accent);
    assert_eq!(cell_fg(&terminal, 9, 0), theme.accent);
}

#[test]
fn accent_underline_is_offset_by_the_rule_origin() {
    let theme = indexed_theme();

    let terminal = draw(20, 1, |frame, _| {
        let rule = Rect::new(4, 0, 10, 1);
        hairline(frame, rule, &theme);
        accent_underline(frame, rule, 0..2, &theme);
    });

    assert_eq!(cell_fg(&terminal, 3, 0), Color::Reset);
    assert_eq!(cell_fg(&terminal, 4, 0), theme.accent);
    assert_eq!(cell_fg(&terminal, 5, 0), theme.accent);
    assert_eq!(cell_fg(&terminal, 6, 0), theme.line);
}

#[test]
fn top_bar_places_left_and_right_lines_at_the_two_edges() {
    let terminal = draw(40, 1, |frame, area| {
        top_bar(frame, area, Line::from("porthmos"), Line::from("status"));
    });

    let row = row_text(&terminal, 0);
    assert!(row.starts_with("porthmos"));
    assert!(row.ends_with("status"));
}

#[test]
fn top_bar_drops_the_right_line_when_it_would_overlap() {
    let terminal = draw(12, 1, |frame, area| {
        top_bar(frame, area, Line::from("porthmos"), Line::from(Span::raw("statusxx")));
    });

    assert_eq!(row_text(&terminal, 0).trim_end(), "porthmos");
}

#[test]
fn hint_bar_renders_key_in_bright_label_in_dim_with_three_cell_gaps() {
    let theme = indexed_theme();

    let terminal = draw(60, 1, |frame, area| hint_bar(frame, area, &sample_hints(), &theme));

    assert_eq!(row_text(&terminal, 0).trim_end(), "F5 copy   F7 mkdir   F1 help");
    assert_eq!(cell_fg(&terminal, 0, 0), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, 3, 0), theme.fg_dim);
}

#[test]
fn hint_bar_drops_entries_from_the_right_but_keeps_the_pinned_pair() {
    let hints = sample_hints();

    assert_eq!(hint_row(60, &hints).trim_end(), "F5 copy   F7 mkdir   F1 help");
    assert_eq!(hint_row(28, &hints).trim_end(), "F5 copy   F7 mkdir   F1 help");
    assert_eq!(hint_row(27, &hints).trim_end(), "F5 copy   F1 help");
    assert_eq!(hint_row(17, &hints).trim_end(), "F5 copy   F1 help");
    assert_eq!(hint_row(16, &hints).trim_end(), "F1 help");
    assert_eq!(hint_row(7, &hints).trim_end(), "F1 help");
}

#[test]
fn hint_bar_never_draws_a_partial_entry() {
    let hints = sample_hints();

    for width in 7..40 {
        let text = hint_row(width, &hints);
        let text = text.trim_end();
        assert!(
            ["F1 help", "F5 copy   F1 help", "F5 copy   F7 mkdir   F1 help"].contains(&text),
            "width {width}: {text:?}"
        );
    }
}

#[test]
fn hint_bar_puts_the_right_pair_at_the_right_edge() {
    let mut hints = sample_hints();
    hints.right = Some(hint("Esc", "back"));

    let row = hint_row(60, &hints);

    assert!(row.starts_with("F5 copy   F7 mkdir   F1 help"));
    assert!(row.ends_with("Esc back"));
}

#[test]
fn hint_bar_drops_the_right_pair_only_when_it_cannot_share_the_row_with_the_pinned_pair() {
    let mut hints = sample_hints();
    hints.right = Some(hint("Esc", "back"));

    let squeezed = hint_row(17, &hints);
    let fitting = hint_row(18, &hints);

    assert!(!squeezed.contains("Esc back"));
    assert!(squeezed.contains("F5 copy"));
    assert!(fitting.ends_with("Esc back"));
    assert!(fitting.contains("F1 help"));
    assert!(!fitting.contains("F5 copy"));
}

#[test]
fn hint_bar_keeps_the_right_pair_while_entries_are_dropped_to_make_room() {
    let mut hints = sample_hints();
    hints.right = Some(hint("Esc", "back"));

    let row = hint_row(28, &hints);

    assert!(row.ends_with("Esc back"));
    assert!(row.contains("F1 help"));
    assert!(!row.contains("F7 mkdir"));
}

#[test]
fn hint_bar_survives_zero_width_and_a_pinned_pair_wider_than_the_area() {
    let hints = sample_hints();

    for width in [0, 1, 3] {
        let _ = hint_row(width, &hints);
    }
    let theme = indexed_theme();
    let _ = draw(5, 1, |frame, _| hint_bar(frame, Rect::new(0, 0, 0, 1), &hints, &theme));
}

#[test]
fn hint_bar_measures_wide_characters_by_display_width() {
    let hints = Hints { entries: vec![hint("一", "one")], pinned: Some(hint("F1", "help")), right: None };

    assert_eq!(hint_row(20, &hints).trim_end(), "一 one   F1 help");
    assert_eq!(hint_row(15, &hints).trim_end(), "F1 help");
}

#[test]
fn hint_bar_does_not_waste_a_gap_before_the_right_pair_when_nothing_is_pinned() {
    let hints = Hints { entries: vec![hint("F5", "copy")], pinned: None, right: Some(hint("Esc", "back")) };

    let row = hint_row(18, &hints);

    assert!(row.starts_with("F5 copy"), "{row:?}");
    assert!(row.ends_with("Esc back"), "{row:?}");
}

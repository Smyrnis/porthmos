use ratatui::{Frame, Terminal, backend::TestBackend, layout::Rect, style::Modifier, text::Line, widgets::Paragraph};

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text},
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn fill(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();
    let rows: Vec<Line> = (0..area.height).map(|_| Line::from("x".repeat(usize::from(area.width)))).collect();
    frame.render_widget(Paragraph::new(rows).style(theme.text()), area);
}

fn draw_popup(width: u16, height: u16, size: (u16, u16), title: Option<&str>) -> (Terminal<TestBackend>, Rect) {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut inner = Rect::default();
    terminal
        .draw(|frame| {
            fill(frame, &theme);
            frame.buffer_mut().set_style(Rect::new(0, 0, width, 1), theme.focus_row());
            inner = render_popup(frame, frame.area(), size, title, &theme);
        })
        .unwrap();
    (terminal, inner)
}

fn row_text(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_string()).collect()
}

#[test]
fn the_backdrop_dims_every_cell_outside_the_popup() {
    let theme = indexed();
    let (terminal, _) = draw_popup(40, 12, (20, 6), None);

    for (x, y) in [(0, 1), (39, 11), (5, 5), (30, 9)] {
        assert_eq!(terminal.backend().buffer()[(x, y)].symbol(), "x");
        assert_eq!(cell_fg(&terminal, x, y), theme.line_strong, "cell {x},{y}");
    }
}

#[test]
fn a_focus_row_background_survives_the_backdrop() {
    let theme = indexed();
    let (terminal, _) = draw_popup(40, 12, (20, 6), None);

    assert_eq!(cell_bg(&terminal, 0, 0), theme.row_focus);
}

#[test]
fn the_popup_is_centred_cleared_and_filled_with_the_background() {
    let theme = indexed();
    let (terminal, inner) = draw_popup(40, 12, (20, 6), None);

    assert_eq!(inner, Rect::new(12, 4, 16, 4));
    for y in inner.top()..inner.bottom() {
        for x in inner.left()..inner.right() {
            assert_eq!(terminal.backend().buffer()[(x, y)].symbol(), " ", "cell {x},{y}");
            assert_eq!(cell_bg(&terminal, x, y), theme.bg);
        }
    }
}

#[test]
fn the_border_is_line_strong_and_the_title_is_muted_between_spaces() {
    let theme = indexed();
    let (terminal, _) = draw_popup(40, 12, (20, 6), Some("Hi"));

    assert_eq!(terminal.backend().buffer()[(10, 3)].symbol(), "\u{250c}");
    assert_eq!(cell_fg(&terminal, 10, 3), theme.line_strong);
    assert_eq!(terminal.backend().buffer()[(11, 3)].symbol(), " ");
    assert_eq!(terminal.backend().buffer()[(12, 3)].symbol(), "H");
    assert_eq!(cell_fg(&terminal, 12, 3), theme.fg_muted);
    assert_eq!(terminal.backend().buffer()[(14, 3)].symbol(), " ");
}

#[test]
fn the_returned_rect_is_inside_the_border_with_one_cell_of_side_padding() {
    let (_, inner) = draw_popup(40, 12, (30, 8), Some("t"));

    assert_eq!(inner, Rect::new(7, 3, 26, 6));
}

#[test]
fn a_popup_larger_than_the_screen_is_clamped_to_it() {
    let (_, inner) = draw_popup(20, 8, (100, 50), None);

    assert_eq!(inner, Rect::new(2, 1, 16, 6));
}

#[test]
fn zero_and_tiny_screens_do_not_panic() {
    for (width, height) in [(0, 0), (1, 1), (3, 2), (4, 3), (0, 5), (5, 0)] {
        let (_, inner) = draw_popup(width, height, (20, 6), Some("title"));
        assert!(inner.right() <= width && inner.bottom() <= height, "{width}x{height}: {inner:?}");
    }
}

#[test]
fn centered_clamps_and_centres() {
    assert_eq!(centered(Rect::new(0, 0, 40, 12), 20, 6), Rect::new(10, 3, 20, 6));
    assert_eq!(centered(Rect::new(2, 1, 10, 4), 30, 9), Rect::new(2, 1, 10, 4));
    assert_eq!(centered(Rect::new(0, 0, 0, 0), 5, 5), Rect::new(0, 0, 0, 0));
}

fn draw_footer(width: u16, pairs: &[(&str, &str)]) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal.draw(|frame| footer(frame, frame.area(), pairs, &theme)).unwrap();
    terminal
}

#[test]
fn footer_pairs_use_bright_keys_dim_labels_and_three_cells_between() {
    let theme = indexed();
    let terminal = draw_footer(40, &[("y", "yes"), ("n", "no")]);

    assert!(row_text(&terminal, 0).starts_with("y yes   n no"));
    assert_eq!(cell_fg(&terminal, 0, 0), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, 2, 0), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, 8, 0), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, 10, 0), theme.fg_dim);
}

#[test]
fn footer_drops_pairs_from_the_right_when_it_is_narrow() {
    let terminal = draw_footer(8, &[("y", "yes"), ("n", "no")]);

    assert_eq!(find_text(&terminal, "y yes"), Some((0, 0)));
    assert_eq!(find_text(&terminal, "n no"), None);
}

#[test]
fn footer_on_an_empty_area_does_not_panic() {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(5, 2)).unwrap();
    terminal.draw(|frame| footer(frame, Rect::new(0, 0, 0, 0), &[("y", "yes")], &theme)).unwrap();
}

#[test]
fn option_spans_show_a_chevron_only_when_focused() {
    let theme = indexed();
    let focused = option_spans("y", "yes", true, &theme);
    let idle = option_spans("y", "yes", false, &theme);

    let text = |spans: &[ratatui::text::Span]| spans.iter().map(|span| span.content.as_ref()).collect::<String>();
    assert_eq!(text(&focused), "\u{203a} y yes");
    assert_eq!(text(&idle), "  y yes");
    assert_eq!(focused[0].style.fg, Some(theme.accent));
    assert_eq!(focused[1].style.fg, Some(theme.fg_bright));
    assert_eq!(focused.last().unwrap().style.fg, Some(theme.fg_bright));
    assert_eq!(idle.last().unwrap().style.fg, Some(theme.fg_dim));
    assert!(!focused.iter().chain(&idle).any(|span| span.style.add_modifier.contains(Modifier::REVERSED)));
}

#[test]
fn content_width_uses_display_width_and_clamps() {
    assert_eq!(content_width(&["\u{65e5}\u{672c}".repeat(6).as_str()]), 28);
    assert_eq!(content_width(&[&"a".repeat(500)]), 76);
    assert_eq!(content_width(&[]), 20);
    assert_eq!(content_width(&["short"]), 20);
    assert_eq!(content_width(&[&"x".repeat(37)]), 41);
    assert_eq!(content_width(&["", "ab"]), 20);
}

#[test]
fn wrap_line_breaks_between_words() {
    assert_eq!(wrap_line("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
    assert_eq!(wrap_line("aaa bbb ccc", 11), vec!["aaa bbb ccc"]);
}

#[test]
fn wrap_line_keeps_an_empty_line() {
    assert_eq!(wrap_line("", 10), vec![String::new()]);
}

#[test]
fn wrap_line_splits_a_long_word_by_display_width() {
    assert_eq!(
        wrap_line("\u{65e5}\u{672c}\u{8a9e}\u{65e5}\u{672c}\u{8a9e}", 5),
        vec!["\u{65e5}\u{672c}", "\u{8a9e}\u{65e5}", "\u{672c}\u{8a9e}"]
    );
    let long = "x".repeat(500);
    let lines = wrap_line(&long, 72);
    assert_eq!(lines.concat(), long);
    assert!(lines.iter().all(|line| line.len() <= 72));
}

#[test]
fn wrap_line_with_no_room_makes_progress() {
    assert_eq!(wrap_line("abc", 0), vec!["a", "b", "c"]);
    assert_eq!(wrap_line("\u{65e5}\u{672c}", 1), vec!["\u{65e5}", "\u{672c}"]);
}

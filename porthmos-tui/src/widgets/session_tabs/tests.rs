use ratatui::{style::Style, text::Line};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::theme::{ColorDepth, Theme, ThemeSettings};

fn indexed_theme() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn tab<'a>(number: usize, name: &'a str, protocol: &'a str, active: bool) -> TabView<'a> {
    TabView { number, name, protocol, active }
}

fn three_tabs() -> Vec<TabView<'static>> {
    vec![tab(1, "prod", "sftp", false), tab(2, "backup", "s3", true), tab(3, "nas", "ftp", false)]
}

fn text(line: &Line) -> String {
    line.spans.iter().map(|span| span.content.as_ref()).collect()
}

fn style_at(line: &Line, column: usize) -> Style {
    let mut start = 0;
    for span in &line.spans {
        let width = span.content.width();
        if column < start + width {
            return span.style;
        }
        start += width;
    }
    panic!("column {column} is past the end of {:?}", text(line));
}

fn column_of(line: &Line, needle: &str) -> usize {
    let text = text(line);
    text[..text.find(needle).expect("needle in line")].width()
}

#[test]
fn no_sessions_make_an_empty_line() {
    let result = tabs_line(&[], 80, &indexed_theme());

    assert_eq!(text(&result.line), "");
    assert_eq!(result.active_columns, None);
}

#[test]
fn one_session_shows_number_name_protocol_and_a_green_dot() {
    let theme = indexed_theme();

    let result = tabs_line(&[tab(1, "prod-web", "sftp", true)], 80, &theme);

    assert_eq!(text(&result.line), "1 prod-web sftp \u{2022}");
    assert_eq!(style_at(&result.line, 0).fg, Some(theme.fg_dim));
    assert_eq!(style_at(&result.line, 2).fg, Some(theme.fg_bright));
    assert_eq!(style_at(&result.line, 11).fg, Some(theme.fg_dim));
    assert_eq!(style_at(&result.line, 16).fg, Some(theme.ok));
}

#[test]
fn a_session_without_a_known_protocol_shows_no_gap_for_it() {
    let result = tabs_line(&[tab(1, "prod", "", true)], 80, &indexed_theme());

    assert_eq!(text(&result.line), "1 prod \u{2022}");
}

#[test]
fn inactive_tabs_use_the_muted_name_colour() {
    let theme = indexed_theme();

    let result = tabs_line(&three_tabs(), 80, &theme);

    assert_eq!(style_at(&result.line, 2).fg, Some(theme.fg_muted));
    assert_eq!(style_at(&result.line, column_of(&result.line, "backup")).fg, Some(theme.fg_bright));
}

#[test]
fn tabs_are_separated_by_three_cells() {
    let result = tabs_line(&three_tabs(), 80, &indexed_theme());

    assert_eq!(text(&result.line), "1 prod sftp \u{2022}   2 backup s3 \u{2022}   3 nas ftp \u{2022}");
}

#[test]
fn active_columns_cover_the_number_and_name_of_the_active_tab_only() {
    let result = tabs_line(&three_tabs(), 80, &indexed_theme());

    let range = result.active_columns.expect("an active tab");
    let full = text(&result.line);
    let covered: String = full.chars().skip(range.start as usize).take((range.end - range.start) as usize).collect();
    assert_eq!(covered, "2 backup");
}

#[test]
fn no_active_tab_means_no_active_columns() {
    let tabs = vec![tab(1, "prod", "sftp", false)];

    assert_eq!(tabs_line(&tabs, 80, &indexed_theme()).active_columns, None);
}

#[test]
fn narrow_widths_drop_protocols_then_inactive_names_then_inactive_tabs() {
    let theme = indexed_theme();
    let cases = [
        (43, "1 prod sftp \u{2022}   2 backup s3 \u{2022}   3 nas ftp \u{2022}"),
        (42, "1 prod \u{2022}   2 backup \u{2022}   3 nas \u{2022}"),
        (31, "1 prod \u{2022}   2 backup \u{2022}   3 nas \u{2022}"),
        (30, "1 \u{2022}   2 backup \u{2022}   3 \u{2022}"),
        (22, "1 \u{2022}   2 backup \u{2022}   3 \u{2022}"),
        (21, "2 backup \u{2022}"),
        (10, "2 backup \u{2022}"),
        (9, "2 back\u{2026} \u{2022}"),
    ];

    for (width, expected) in cases {
        let result = tabs_line(&three_tabs(), width, &theme);
        assert_eq!(text(&result.line), expected, "width {width}");
    }
}

#[test]
fn the_active_columns_follow_the_degraded_layout() {
    let result = tabs_line(&three_tabs(), 30, &indexed_theme());

    let range = result.active_columns.expect("an active tab");
    let full = text(&result.line);
    let covered: String = full.chars().skip(range.start as usize).take((range.end - range.start) as usize).collect();
    assert_eq!(covered, "2 backup");
}

#[test]
fn a_five_hundred_character_name_is_truncated_with_an_ellipsis_and_never_exceeds_the_width() {
    let name = "x".repeat(500);
    let tabs = [tab(1, &name, "sftp", true)];

    for width in [6, 12, 40, 80] {
        let result = tabs_line(&tabs, width, &indexed_theme());
        let shown = text(&result.line);
        assert!(shown.width() <= usize::from(width), "width {width}: {shown:?}");
        assert!(shown.contains('\u{2026}'), "width {width}: {shown:?}");
    }
}

#[test]
fn wide_and_combining_characters_are_measured_by_display_width() {
    let theme = indexed_theme();
    let tabs = [tab(1, "\u{4e00}\u{4e8c}\u{4e09}\u{56db}\u{4e94}", "sftp", true)];

    for width in 1..30u16 {
        let result = tabs_line(&tabs, width, &theme);
        assert!(text(&result.line).width() <= usize::from(width), "width {width}");
    }
    let combining = [tab(1, "e\u{301}e\u{301}e\u{301}", "", true)];
    assert!(text(&tabs_line(&combining, 6, &theme).line).width() <= 6);
}

#[test]
fn control_and_bidi_characters_in_names_are_replaced() {
    let tabs = [tab(1, "a\u{202e}b\nc", "sftp", true)];

    let shown = text(&tabs_line(&tabs, 80, &indexed_theme()).line);

    assert!(shown.contains("a?b?c"), "{shown:?}");
    assert!(!shown.contains('\u{202e}'));
}

#[test]
fn twenty_sessions_fit_in_eighty_columns_with_the_active_tab_visible() {
    let names: Vec<String> = (1..=20).map(|n| format!("server-{n}")).collect();
    let tabs: Vec<TabView> = names.iter().enumerate().map(|(i, name)| tab(i + 1, name, "sftp", i == 6)).collect();

    let result = tabs_line(&tabs, 80, &indexed_theme());

    assert!(text(&result.line).width() <= 80);
    assert!(text(&result.line).contains("7 server-7"));
    assert!(result.active_columns.is_some());
}

#[test]
fn zero_and_tiny_widths_do_not_panic() {
    for width in 0..5 {
        let result = tabs_line(&three_tabs(), width, &indexed_theme());
        assert!(text(&result.line).width() <= usize::from(width));
    }
}

#[test]
fn the_active_tab_stays_visible_when_it_is_the_last_of_many() {
    let names: Vec<String> = (1..=20).map(|n| format!("server-{n}")).collect();
    let tabs: Vec<TabView> = names.iter().enumerate().map(|(i, name)| tab(i + 1, name, "sftp", i == 19)).collect();

    let result = tabs_line(&tabs, 80, &indexed_theme());

    assert!(text(&result.line).contains("20 server-20"));
}

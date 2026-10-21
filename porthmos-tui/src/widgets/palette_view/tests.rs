use ratatui::{Frame, Terminal, backend::TestBackend, style::Modifier, text::Line, widgets::Paragraph};

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text, has_unprintable_cell, screen_rows},
    input::Action,
    palette::{Entry, Group, Target},
    theme::{ColorDepth, Theme, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn action(label: &str, hint: Option<&str>) -> Entry {
    Entry {
        group: Group::Actions,
        label: label.to_string(),
        hint: hint.map(str::to_string),
        target: Target::Action(Action::Quit),
    }
}

fn host(label: &str, hint: &str) -> Entry {
    Entry {
        group: Group::Hosts,
        label: label.to_string(),
        hint: Some(hint.to_string()),
        target: Target::Connect(label.to_string()),
    }
}

fn typed(entries: Vec<Entry>, query: &str) -> Palette {
    let mut palette = Palette::new(entries);
    for character in query.chars() {
        palette.type_char(character);
    }
    palette
}

fn sample() -> Palette {
    Palette::new(vec![
        action("New connection", Some("n")),
        action("Connections", Some("F9")),
        action("Disconnect prod-web", None),
        host("Connect nas", "smb"),
        host("Connect staging", "sftp"),
    ])
}

fn draw(palette: &Palette, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_palette(frame, frame.area(), palette, &theme)).unwrap();
    terminal
}

fn fill(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();
    let rows: Vec<Line> = (0..area.height).map(|_| Line::from("x".repeat(usize::from(area.width)))).collect();
    frame.render_widget(Paragraph::new(rows).style(theme.text()), area);
}

fn corners(terminal: &Terminal<TestBackend>) -> (u16, u16, u16, u16) {
    let rows = screen_rows(terminal);
    let top = rows.iter().position(|row| row.contains('\u{250c}')).unwrap() as u16;
    let bottom = rows.iter().position(|row| row.contains('\u{2514}')).unwrap() as u16;
    let left = rows[usize::from(top)].chars().position(|character| character == '\u{250c}').unwrap() as u16;
    let right = rows[usize::from(top)].chars().position(|character| character == '\u{2510}').unwrap() as u16;
    (left, top, right, bottom)
}

#[test]
fn the_popup_is_sixty_wide_centred_and_a_sixth_from_the_top() {
    let terminal = draw(&sample(), 120, 36);

    let (left, top, right, _) = corners(&terminal);
    assert_eq!((left, right), (30, 89));
    assert_eq!(top, 6);
}

#[test]
fn at_sixty_six_columns_and_below_the_popup_is_ninety_percent_wide() {
    let (left, _, right, _) = corners(&draw(&sample(), 66, 30));
    assert_eq!(right - left + 1, 59);

    let (left, _, right, _) = corners(&draw(&sample(), 67, 30));
    assert_eq!(right - left + 1, 60);

    let (left, _, right, _) = corners(&draw(&sample(), 50, 30));
    assert_eq!(right - left + 1, 45);
}

#[test]
fn the_screen_behind_is_dimmed() {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

    terminal
        .draw(|frame| {
            fill(frame, &theme);
            render_palette(frame, frame.area(), &sample(), &theme);
        })
        .unwrap();

    assert_eq!(cell_fg(&terminal, 0, 0), theme.line_strong);
    assert_eq!(cell_fg(&terminal, 99, 29), theme.line_strong);
}

#[test]
fn the_input_row_shows_a_chevron_the_query_and_the_match_count() {
    let theme = indexed();
    let palette =
        typed(vec![action("New connection", None), action("Connections", None), host("Connect nas", "smb")], "conn");
    let terminal = draw(&palette, 100, 30);

    let (left, top, right, _) = corners(&terminal);
    let y = top + 1;
    let row = &screen_rows(&terminal)[usize::from(y)];
    assert!(row.contains("\u{203a} conn"), "{row:?}");
    let (x, _) = find_text(&terminal, "\u{203a} conn").unwrap();
    assert_eq!(x, left + 2);
    assert_eq!(cell_fg(&terminal, x, y), theme.accent);
    assert_eq!(cell_fg(&terminal, x + 2, y), theme.fg_bright);
    assert_eq!(terminal.backend().buffer()[(right - 2, y)].symbol(), "3");
    assert_eq!(cell_fg(&terminal, right - 2, y), theme.fg_dim);
}

#[test]
fn the_terminal_cursor_sits_at_the_caret_in_display_width() {
    let mut palette = typed(vec![action("\u{65e5}\u{672c}", None)], "\u{65e5}\u{672c}x");
    palette.move_caret(-1);

    let mut terminal = draw(&palette, 100, 30);

    let (left, top, _, _) = corners(&terminal);
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (left + 2 + 2 + 4, top + 1));
}

#[test]
fn only_line_hairlines_separate_the_input_the_results_and_the_footer() {
    let theme = indexed();
    let terminal = draw(&sample(), 100, 30);

    let (left, top, right, bottom) = corners(&terminal);
    for y in [top + 2, bottom - 2] {
        for x in (left + 2)..(right - 1) {
            assert_eq!(terminal.backend().buffer()[(x, y)].symbol(), "\u{2500}", "{x},{y}");
            assert_eq!(cell_fg(&terminal, x, y), theme.line, "{x},{y}");
        }
    }
}

#[test]
fn group_titles_are_dim_lowercase_with_a_blank_row_above_all_but_the_first() {
    let theme = indexed();
    let terminal = draw(&sample(), 100, 30);

    let (x, actions_row) = find_text(&terminal, "actions").unwrap();
    let (_, hosts_row) = find_text(&terminal, "hosts").unwrap();
    assert_eq!(cell_fg(&terminal, x, actions_row), theme.fg_dim);
    assert_eq!(cell_fg(&terminal, x, hosts_row), theme.fg_dim);
    let (_, top, _, _) = corners(&terminal);
    assert_eq!(actions_row, top + 3);
    assert_eq!(hosts_row, actions_row + 3 + 2);
    let between = &screen_rows(&terminal)[usize::from(hosts_row) - 1];
    assert!(between.trim_matches(|c: char| c == '\u{2502}' || c == ' ').is_empty(), "{between:?}");
}

#[test]
fn a_result_shows_its_label_and_a_right_aligned_dim_hint() {
    let theme = indexed();
    let terminal = draw(&sample(), 100, 30);

    let (_, _, right, _) = corners(&terminal);
    let (hint_x, hint_y) = find_text(&terminal, "sftp").unwrap();
    assert_eq!(hint_x + 4, right - 1);
    assert_eq!(cell_fg(&terminal, hint_x, hint_y), theme.fg_dim);
    assert!(screen_rows(&terminal)[usize::from(hint_y)].contains("Connect staging"));
    let (f9_x, _) = find_text(&terminal, "F9").unwrap();
    assert_eq!(f9_x + 2, right - 1);
}

#[test]
fn matched_characters_are_accent_coloured() {
    let theme = indexed();
    let palette = typed(vec![action("New connection", None), action("Nice cup", None)], "nc");
    let terminal = draw(&palette, 100, 30);

    let (x, y) = find_text(&terminal, "Nice cup").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.accent);
    assert_eq!(cell_fg(&terminal, x + 1, y), theme.fg);
    assert_eq!(cell_fg(&terminal, x + 5, y), theme.accent);
    assert_eq!(cell_fg(&terminal, x + 6, y), theme.fg);
}

#[test]
fn the_focused_result_has_a_chevron_the_focus_background_and_a_bold_label() {
    let theme = indexed();
    let mut palette = sample();
    palette.move_selection(1);
    let terminal = draw(&palette, 100, 30);

    let (x, y) = find_text(&terminal, "Connections").unwrap();
    let (left, _, right, _) = corners(&terminal);
    assert_eq!(terminal.backend().buffer()[(x - 2, y)].symbol(), "\u{203a}");
    assert_eq!(cell_fg(&terminal, x - 2, y), theme.accent);
    for column in (left + 2)..(right - 1) {
        assert_eq!(cell_bg(&terminal, column, y), theme.row_focus, "{column}");
    }
    assert!(terminal.backend().buffer()[(x + 5, y)].modifier.contains(Modifier::BOLD));
    assert_eq!(cell_fg(&terminal, x + 5, y), theme.fg_focus);
}

#[test]
fn an_unfocused_result_has_no_chevron_and_no_focus_background() {
    let theme = indexed();
    let terminal = draw(&sample(), 100, 30);

    let (x, y) = find_text(&terminal, "Connections").unwrap();
    assert_eq!(terminal.backend().buffer()[(x - 2, y)].symbol(), " ");
    assert_ne!(cell_bg(&terminal, x, y), theme.row_focus);
    assert_eq!(cell_fg(&terminal, x + 5, y), theme.fg);
}

#[test]
fn the_footer_lists_move_run_and_close_on_the_left_and_the_prefixes_on_the_right() {
    let theme = indexed();
    let terminal = draw(&sample(), 100, 30);

    let (left, _, right, bottom) = corners(&terminal);
    let y = bottom - 1;
    let (move_x, move_y) = find_text(&terminal, "\u{2191}\u{2193} move").unwrap();
    assert_eq!((move_x, move_y), (left + 2, y));
    assert_eq!(cell_fg(&terminal, move_x, y), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, move_x + 3, y), theme.fg_dim);
    assert!(find_text(&terminal, "enter run").is_some());
    assert!(find_text(&terminal, "esc close").is_some());
    let (prefix_x, prefix_y) = find_text(&terminal, "> actions @ hosts # files").unwrap();
    assert_eq!(prefix_y, y);
    assert_eq!(prefix_x + 25, right - 1);
}

#[test]
fn the_prefix_hint_is_dropped_before_the_left_pairs_when_narrow() {
    let terminal = draw(&sample(), 40, 20);

    assert!(find_text(&terminal, "esc close").is_some());
    assert!(find_text(&terminal, "# files").is_none());
}

#[test]
fn no_results_shows_a_dim_no_matches() {
    let theme = indexed();
    let terminal = draw(&typed(vec![action("Copy", None)], "zzz"), 100, 30);

    let (x, y) = find_text(&terminal, "no matches").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    let (_, top, _, bottom) = corners(&terminal);
    assert_eq!(y, top + 3);
    assert_eq!(find_text(&terminal, "actions").map(|(_, row)| row), Some(bottom - 1));
    let (_, top, right, _) = corners(&terminal);
    assert_eq!(terminal.backend().buffer()[(right - 2, top + 1)].symbol(), "0");
}

fn many() -> Palette {
    Palette::new((0..200).map(|index| action(&format!("item{index:03}"), None)).collect())
}

#[test]
fn a_long_result_list_scrolls_to_keep_the_focused_result_visible() {
    let mut palette = many();
    let top = draw(&palette, 100, 30);
    assert!(find_text(&top, "item000").is_some());
    assert!(find_text(&top, "actions").is_some());
    assert!(find_text(&top, "item199").is_none());

    palette.move_selection(150);
    let middle = draw(&palette, 100, 30);
    assert!(find_text(&middle, "item150").is_some());
    assert!(find_text(&middle, "item000").is_none());
    assert!(find_text(&middle, "\u{203a} item150").is_some());

    palette.move_selection(-1000);
    assert!(find_text(&draw(&palette, 100, 30), "item000").is_some());
    palette.move_selection(1000);
    assert!(find_text(&draw(&palette, 100, 30), "item199").is_some());
}

#[test]
fn the_popup_height_grows_with_the_results_up_to_sixty_percent_of_the_screen() {
    let small = corners(&draw(&Palette::new(vec![action("a", None), action("b", None)]), 100, 40));
    assert_eq!(small.3 - small.1 + 1, 9);

    let large = corners(&draw(&many(), 100, 40));
    assert_eq!(large.3 - large.1 + 1, 24);
}

#[test]
fn hostile_labels_and_hints_are_cut_by_display_width_and_never_overflow() {
    let long = "\u{65e5}\u{672c}\u{1b}[2J\u{202e}".repeat(100);
    let palette = Palette::new(vec![
        action(&long, Some(&"h".repeat(40))),
        action(&"x".repeat(500), None),
        host("Connect \u{7}nas", "s\u{1b}ftp"),
    ]);

    let terminal = draw(&palette, 80, 24);

    assert!(!has_unprintable_cell(&terminal));
    let (left, _, right, _) = corners(&terminal);
    let buffer = terminal.backend().buffer();
    for y in 0..24 {
        for x in (0..left).chain((right + 1)..80) {
            assert_eq!(buffer[(x, y)].symbol(), " ", "{x},{y}");
        }
    }
    let (hint_x, hint_y) = find_text(&terminal, "hhh").unwrap();
    assert!(hint_x < right);
    let hint_row = &screen_rows(&terminal)[usize::from(hint_y)];
    assert!(hint_row.trim_end().trim_end_matches('\u{2502}').trim_end().ends_with(['h', '\u{2026}']), "{hint_row:?}");
}

#[test]
fn an_indexed_theme_keeps_focus_match_and_dim_distinct() {
    let theme = indexed();
    let colors = [theme.row_focus, theme.accent, theme.fg_dim, theme.fg, theme.fg_focus];
    for (index, color) in colors.iter().enumerate() {
        assert!(colors.iter().skip(index + 1).all(|other| other != color));
    }
}

#[test]
fn tiny_screens_do_not_panic() {
    let palettes = [sample(), many(), typed(vec![action("a", None)], "zzz"), Palette::new(Vec::new())];
    let widths = (0..=12).chain([40, 59, 60, 61, 67, 90, 130]);
    for width in widths {
        for height in 0..=14 {
            for palette in &palettes {
                draw(palette, width, height);
            }
        }
    }
}

#[test]
fn the_source_uses_no_reversed_modifier() {
    assert!(!include_str!("mod.rs").contains("REVERSED"));
}

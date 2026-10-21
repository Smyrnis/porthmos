use ratatui::{Frame, Terminal, backend::TestBackend, text::Line, widgets::Paragraph};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn row(key: &str, action: &'static str) -> HelpRow {
    HelpRow { key: Some(key.to_string()), action }
}

fn small() -> Vec<HelpSection> {
    vec![
        HelpSection { title: "files", rows: vec![row("F5", "copy"), HelpRow { key: None, action: "quit" }] },
        HelpSection { title: "sync", rows: vec![row("Enter", "run")] },
    ]
}

fn long() -> Vec<HelpSection> {
    vec![HelpSection {
        title: "files",
        rows: (0..40).map(|_| row("F5", "copy")).chain([row("F1", "last_row")]).collect(),
    }]
}

fn draw(sections: &[HelpSection], scroll: usize, width: u16, height: u16) -> (Terminal<TestBackend>, usize) {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut max_scroll = 0;
    terminal.draw(|frame| max_scroll = render_help(frame, frame.area(), sections, scroll, &theme)).unwrap();
    (terminal, max_scroll)
}

fn fill(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();
    let rows: Vec<Line> = (0..area.height).map(|_| Line::from("x".repeat(usize::from(area.width)))).collect();
    frame.render_widget(Paragraph::new(rows).style(theme.text()), area);
}

#[test]
fn section_titles_are_dim_with_a_blank_row_between_sections() {
    let theme = indexed();
    let (terminal, _) = draw(&small(), 0, 70, 20);

    let (x, files_row) = find_text(&terminal, "files").unwrap();
    assert_eq!(cell_fg(&terminal, x, files_row), theme.fg_dim);
    let (_, sync_row) = find_text(&terminal, "sync").unwrap();
    assert_eq!(sync_row, files_row + 4);
    let blank = &screen_rows(&terminal)[usize::from(sync_row) - 1];
    assert!(blank.trim_matches(|c: char| c == '\u{2502}' || c == ' ').is_empty(), "{blank:?}");
}

#[test]
fn keys_are_bright_in_a_twelve_cell_column_and_actions_are_text_coloured() {
    let theme = indexed();
    let (terminal, _) = draw(&small(), 0, 70, 20);

    let (x, y) = find_text(&terminal, "F5").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    let (action_x, action_y) = find_text(&terminal, "copy").unwrap();
    assert_eq!((action_x - x, action_y), (12, y));
    assert_eq!(cell_fg(&terminal, action_x, action_y), theme.fg);
}

#[test]
fn an_unbound_action_shows_a_dim_unbound() {
    let theme = indexed();
    let (terminal, _) = draw(&small(), 0, 70, 20);

    let (x, y) = find_text(&terminal, "unbound").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn the_popup_dims_the_screen_behind_it() {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(70, 20)).unwrap();
    terminal
        .draw(|frame| {
            fill(frame, &theme);
            render_help(frame, frame.area(), &small(), 0, &theme);
        })
        .unwrap();

    assert_eq!(cell_fg(&terminal, 0, 0), theme.line_strong);
}

#[test]
fn the_footer_says_scroll_and_any_key_close() {
    let (terminal, _) = draw(&small(), 0, 70, 20);

    assert!(find_text(&terminal, "any key close").is_some());
    assert!(find_text(&terminal, "scroll").is_some());
}

#[test]
fn the_popup_is_at_most_sixty_four_wide() {
    let (terminal, _) = draw(&small(), 0, 120, 20);

    let (left, _) = find_text(&terminal, "\u{250c}").unwrap();
    let row = &screen_rows(&terminal)[usize::from(find_text(&terminal, "\u{250c}").unwrap().1)];
    let right = row.chars().position(|character| character == '\u{2510}').unwrap();
    assert_eq!(right - usize::from(left) + 1, 64);
}

#[test]
fn everything_fitting_means_no_scrolling() {
    let (_, max_scroll) = draw(&small(), 0, 70, 20);

    assert_eq!(max_scroll, 0);
}

#[test]
fn scrolling_shows_later_rows_and_clamps_at_the_end() {
    let (top, max_scroll) = draw(&long(), 0, 70, 16);
    assert!(max_scroll > 0);
    assert!(find_text(&top, "last_row").is_none());
    assert!(find_text(&top, "files").is_some());

    let (end, _) = draw(&long(), max_scroll, 70, 16);
    assert!(find_text(&end, "last_row").is_some());
    assert!(find_text(&end, "files").is_none());

    let (beyond, again) = draw(&long(), max_scroll + 100, 70, 16);
    assert_eq!(again, max_scroll);
    assert!(find_text(&beyond, "last_row").is_some());
}

#[test]
fn a_ten_row_terminal_can_reach_the_last_row() {
    let (_, max_scroll) = draw(&long(), 0, 70, 10);

    let (terminal, _) = draw(&long(), max_scroll, 70, 10);

    assert!(find_text(&terminal, "last_row").is_some());
    assert!(find_text(&terminal, "any key close").is_some());
}

#[test]
fn hostile_key_text_is_sanitised() {
    let sections =
        vec![HelpSection { title: "files", rows: vec![HelpRow { key: Some("A\u{1b}[2J".to_string()), action: "x" }] }];

    let (terminal, _) = draw(&sections, 0, 70, 20);

    assert!(!crate::app::testing::has_unprintable_cell(&terminal));
}

#[test]
fn tiny_screens_do_not_panic() {
    for width in 0..=12 {
        for height in 0..=6 {
            draw(&small(), 0, width, height);
            draw(&long(), 100, width, height);
            draw(&[], 0, width, height);
        }
    }
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier_or_a_list_box() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
    assert!(!source.contains("Borders"));
}

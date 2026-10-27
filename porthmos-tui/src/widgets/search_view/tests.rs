use porthmos_core::VfsPath;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use porthmos_core::Entry;
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_bg, cell_fg, find_text, has_unprintable_cell, screen_rows},
    theme::{ColorDepth, ThemeSettings},
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn entry(name: &str) -> Entry {
    Entry {
        modified: None,
        name: name.to_string(),
        path: VfsPath::from(format!("/{name}")),
        is_dir: false,
        size: 0,
        permissions: None,
        ..Default::default()
    }
}

#[test]
fn typing_inserts_at_the_cursor_and_reports_pattern_changed() {
    let mut view = SearchView::new();
    assert_eq!(view.handle_key(key(KeyCode::Char('a'))), SearchOutcome::PatternChanged);
    assert_eq!(view.pattern, "a");
    assert_eq!(view.cursor, 1);
}

#[test]
fn ctrl_modified_characters_are_not_inserted() {
    let mut view = SearchView::new();
    let event = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    assert_eq!(view.handle_key(event), SearchOutcome::Pending);
    assert_eq!(view.pattern, "");
}

#[test]
fn down_and_up_move_the_selection_within_bounds() {
    let mut view = SearchView::new();
    view.push_result(entry("a"));
    view.push_result(entry("b"));

    view.handle_key(key(KeyCode::Down));
    assert_eq!(view.selected, 1);
    view.handle_key(key(KeyCode::Down));
    assert_eq!(view.selected, 1);
    view.handle_key(key(KeyCode::Up));
    assert_eq!(view.selected, 0);
}

#[test]
fn start_clears_previous_results_and_state() {
    let mut view = SearchView::new();
    view.push_result(entry("a"));
    view.finish(true);

    view.start();

    assert!(view.results.is_empty());
    assert!(!view.truncated);
    assert_eq!(view.selected, 0);
}

#[test]
fn esc_reports_cancel() {
    let mut view = SearchView::new();
    assert_eq!(view.handle_key(key(KeyCode::Esc)), SearchOutcome::Cancel);
}

#[test]
fn enter_reports_open() {
    let mut view = SearchView::new();
    assert_eq!(view.handle_key(key(KeyCode::Enter)), SearchOutcome::Open);
}

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(view: &SearchView, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_search(frame, frame.area(), view, &theme)).unwrap();
    terminal
}

fn typed(pattern: &str) -> SearchView {
    let mut view = SearchView::new();
    for character in pattern.chars() {
        view.handle_key(key(KeyCode::Char(character)));
    }
    view
}

#[test]
fn the_first_row_is_a_slash_the_pattern_and_the_result_count() {
    let theme = indexed();
    let mut view = typed("ab");
    view.push_result(entry("ab1"));
    view.push_result(entry("ab2"));

    let terminal = draw(&view, 60, 10);

    let row = &screen_rows(&terminal)[0];
    assert!(row.starts_with("/ ab"), "{row:?}");
    assert!(row.trim_end().ends_with('2'), "{row:?}");
    assert_eq!(cell_fg(&terminal, 0, 0), theme.accent);
    assert_eq!(cell_fg(&terminal, 2, 0), theme.fg_bright);
    assert_eq!(cell_fg(&terminal, 59, 0), theme.fg_dim);
}

#[test]
fn the_count_shows_a_plus_when_the_results_were_truncated() {
    let mut view = typed("ab");
    view.push_result(entry("ab1"));
    view.finish(true);

    let terminal = draw(&view, 60, 10);

    assert!(screen_rows(&terminal)[0].trim_end().ends_with("1+"));
}

#[test]
fn an_empty_pattern_shows_a_dim_type_to_search_row() {
    let theme = indexed();
    let terminal = draw(&SearchView::new(), 60, 10);

    let (x, y) = find_text(&terminal, "type to search").unwrap();
    assert_eq!(y, 2);
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn the_caret_is_the_terminal_cursor_in_display_width() {
    let mut view = typed("\u{65e5}\u{672c}");
    view.handle_key(key(KeyCode::Left));

    let mut terminal = draw(&view, 60, 10);

    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (2 + 2, 0));
}

#[test]
fn a_long_pattern_scrolls_so_the_caret_stays_visible() {
    let pattern: String = (0..100).map(|index| char::from(b'a' + (index % 26) as u8)).collect();
    let view = typed(&pattern);

    let mut terminal = draw(&view, 40, 10);

    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!(cursor.y, 0);
    assert!(cursor.x < 40);
    let last = pattern.chars().last().unwrap().to_string();
    assert_eq!(terminal.backend().buffer()[(cursor.x - 1, 0)].symbol(), last);
}

#[test]
fn an_accent_rule_follows_the_pattern_row() {
    let theme = indexed();
    let terminal = draw(&typed("a"), 60, 10);

    assert_eq!(terminal.backend().buffer()[(0, 1)].symbol(), "\u{2500}");
    assert_eq!(cell_fg(&terminal, 0, 1), theme.accent);
    assert_eq!(cell_fg(&terminal, 59, 1), theme.accent);
}

#[test]
fn the_selected_result_has_a_chevron_and_the_focus_background() {
    let theme = indexed();
    let mut view = typed("a");
    for name in ["a1", "a2", "a3"] {
        view.push_result(entry(name));
    }
    view.handle_key(key(KeyCode::Down));

    let terminal = draw(&view, 60, 10);

    let (x, y) = find_text(&terminal, "/a2").unwrap();
    assert_eq!(y, 3);
    assert_eq!(terminal.backend().buffer()[(0, y)].symbol(), "\u{203a}");
    assert_eq!(cell_fg(&terminal, 0, y), theme.accent);
    assert_eq!(cell_bg(&terminal, x, y), theme.row_focus);
    assert_ne!(cell_bg(&terminal, x, y - 1), theme.row_focus);
}

#[test]
fn the_directory_part_is_dim_and_the_file_name_is_text_coloured() {
    let theme = indexed();
    let mut view = typed("a");
    let mut found = entry("a.txt");
    found.path = VfsPath::from("/srv/site/a.txt");
    view.push_result(found);
    view.push_result(entry("b.txt"));
    view.handle_key(key(KeyCode::Down));

    let terminal = draw(&view, 60, 10);

    let (x, y) = find_text(&terminal, "/srv/site/").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
    let (x, y) = find_text(&terminal, "a.txt").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg);
}

#[test]
fn no_results_for_a_pattern_says_no_matches() {
    let theme = indexed();
    let terminal = draw(&typed("zz"), 60, 10);

    let (x, y) = find_text(&terminal, "no matches").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_dim);
}

#[test]
fn results_scroll_with_the_selection() {
    let mut view = typed("n");
    for index in 0..40 {
        view.push_result(entry(&format!("n{index:02}")));
    }
    for _ in 0..35 {
        view.handle_key(key(KeyCode::Down));
    }

    let terminal = draw(&view, 60, 10);

    assert!(find_text(&terminal, "/n35").is_some());
    assert!(find_text(&terminal, "/n00").is_none());
}

#[test]
fn hostile_and_very_long_paths_are_sanitised_and_cut() {
    let mut view = typed("a");
    let mut hostile = entry("x");
    hostile.path = VfsPath::from(format!("/{}", "d\u{1b}[2J\u{202e}\u{65e5}/".repeat(80)));
    view.push_result(hostile);
    let mut long_name = entry("y");
    long_name.path = VfsPath::from(format!("/dir/{}", "n".repeat(500)));
    view.push_result(long_name);

    let terminal = draw(&view, 60, 10);

    assert!(!has_unprintable_cell(&terminal));
    assert!(screen_rows(&terminal)[3].contains('\u{2026}'));
}

#[test]
fn tiny_screens_do_not_panic() {
    let mut view = typed("a");
    view.push_result(entry("a1"));
    for width in 0..=12 {
        for height in 0..=4 {
            draw(&view, width, height);
            draw(&SearchView::new(), width, height);
        }
    }
}

#[test]
fn letters_digits_and_question_marks_only_edit_the_pattern() {
    let mut view = SearchView::new();
    for character in "q1?d ".chars() {
        assert_eq!(view.handle_key(key(KeyCode::Char(character))), SearchOutcome::PatternChanged);
    }
    assert_eq!(view.pattern, "q1?d ");
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier_or_a_box() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("REVERSED"));
    assert!(!source.contains("Borders"));
}

use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

use super::*;
use crate::{
    app::testing::{cell_fg, find_text, has_unprintable_cell},
    theme::{ColorDepth, ThemeSettings},
};

fn indexed() -> Theme {
    Theme::new(ColorDepth::Indexed, &ThemeSettings::default())
}

fn draw(dialog: &TextInputDialog, width: u16, height: u16) -> Terminal<TestBackend> {
    let theme = indexed();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_text_input(frame, frame.area(), dialog, &theme)).unwrap();
    terminal
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

#[test]
fn typing_characters_appends_to_value() {
    let mut dialog = TextInputDialog::new("New name", "");
    dialog.handle_key(key(KeyCode::Char('a')));
    dialog.handle_key(key(KeyCode::Char('b')));
    assert_eq!(dialog.value, "ab");
}

#[test]
fn backspace_removes_the_last_character() {
    let mut dialog = TextInputDialog::new("New name", "ab");
    dialog.handle_key(key(KeyCode::Backspace));
    assert_eq!(dialog.value, "a");
}

#[test]
fn enter_submits_the_current_value() {
    let mut dialog = TextInputDialog::new("New name", "final");
    let outcome = dialog.handle_key(key(KeyCode::Enter));
    assert_eq!(outcome, TextInputOutcome::Submitted("final".to_string()));
}

#[test]
fn esc_cancels() {
    let mut dialog = TextInputDialog::new("New name", "final");
    let outcome = dialog.handle_key(key(KeyCode::Esc));
    assert_eq!(outcome, TextInputOutcome::Cancelled);
}

#[test]
fn left_and_right_move_the_cursor_without_changing_the_value() {
    let mut dialog = TextInputDialog::new("Name", "abc");
    assert_eq!(dialog.cursor, 3);

    dialog.handle_key(key(KeyCode::Left));
    dialog.handle_key(key(KeyCode::Left));
    assert_eq!(dialog.cursor, 1);

    dialog.handle_key(key(KeyCode::Right));
    assert_eq!(dialog.cursor, 2);
    assert_eq!(dialog.value, "abc");
}

#[test]
fn home_and_end_jump_to_the_boundaries() {
    let mut dialog = TextInputDialog::new("Name", "abc");
    dialog.handle_key(key(KeyCode::Home));
    assert_eq!(dialog.cursor, 0);
    dialog.handle_key(key(KeyCode::End));
    assert_eq!(dialog.cursor, 3);
}

#[test]
fn typing_inserts_at_the_cursor_not_only_at_the_end() {
    let mut dialog = TextInputDialog::new("Name", "ac");
    dialog.cursor = 1;
    dialog.handle_key(key(KeyCode::Char('b')));
    assert_eq!(dialog.value, "abc");
    assert_eq!(dialog.cursor, 2);
}

#[test]
fn delete_removes_the_character_at_the_cursor() {
    let mut dialog = TextInputDialog::new("Name", "abc");
    dialog.cursor = 0;
    dialog.handle_key(key(KeyCode::Delete));
    assert_eq!(dialog.value, "bc");
    assert_eq!(dialog.cursor, 0);
}

#[test]
fn backspace_at_the_start_does_nothing() {
    let mut dialog = TextInputDialog::new("Name", "abc");
    dialog.cursor = 0;
    dialog.handle_key(key(KeyCode::Backspace));
    assert_eq!(dialog.value, "abc");
    assert_eq!(dialog.cursor, 0);
}

#[test]
fn cursor_stays_within_bounds_on_an_empty_value() {
    let mut dialog = TextInputDialog::new("Name", "");
    dialog.handle_key(key(KeyCode::Left));
    assert_eq!(dialog.cursor, 0);
    dialog.handle_key(key(KeyCode::Right));
    assert_eq!(dialog.cursor, 0);
}

#[test]
fn the_value_sits_on_an_underline_with_the_terminal_cursor_at_the_caret() {
    let theme = indexed();
    let mut terminal = draw(&TextInputDialog::new("Rename to", "abc"), 60, 12);

    let (x, y) = find_text(&terminal, "abc").unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (x + 3, y));
    assert_eq!(terminal.backend().buffer()[(x, y + 1)].symbol(), "\u{2500}");
    assert_eq!(cell_fg(&terminal, x, y + 1), theme.accent);
}

#[test]
fn the_caret_column_uses_display_width_for_wide_characters() {
    let mut dialog = TextInputDialog::new("Name", "\u{65e5}\u{672c}");
    dialog.cursor = 1;

    let mut terminal = draw(&dialog, 60, 12);

    let (x, y) = find_text(&terminal, "\u{65e5}").unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    assert_eq!((cursor.x, cursor.y), (x + 2, y));
}

#[test]
fn a_long_value_scrolls_so_the_caret_stays_visible() {
    let value: String = (0..200).map(|index| char::from(b'a' + (index % 26) as u8)).collect();
    let dialog = TextInputDialog::new("Name", value.clone());

    let mut terminal = draw(&dialog, 60, 12);

    let cursor = terminal.get_cursor_position().unwrap();
    let last = value.chars().last().unwrap().to_string();
    assert_eq!(terminal.backend().buffer()[(cursor.x - 1, cursor.y)].symbol(), last);
    assert!(cursor.x < 58);
}

#[test]
fn the_footer_says_enter_ok_and_esc_cancel() {
    let theme = indexed();
    let terminal = draw(&TextInputDialog::new("Name", ""), 60, 12);

    let (x, y) = find_text(&terminal, "enter ok").unwrap();
    assert_eq!(cell_fg(&terminal, x, y), theme.fg_bright);
    assert!(find_text(&terminal, "esc cancel").is_some());
}

#[test]
fn typing_letters_digits_and_question_marks_only_edits_the_value() {
    let mut dialog = TextInputDialog::new("Name", "");
    for character in "q1?d ".chars() {
        assert_eq!(dialog.handle_key(key(KeyCode::Char(character))), TextInputOutcome::Pending);
    }
    assert_eq!(dialog.value, "q1?d ");
}

#[test]
fn hostile_title_and_value_are_sanitised() {
    let dialog = TextInputDialog::new("T\u{1b}[2J", "a\u{1b}b\u{202e}c");

    let terminal = draw(&dialog, 60, 12);

    assert!(!has_unprintable_cell(&terminal));
    assert!(find_text(&terminal, "a?b?c").is_some());
}

#[test]
fn tiny_screens_do_not_panic() {
    for width in 0..=12 {
        for height in 0..=7 {
            draw(&TextInputDialog::new("Name", "value"), width, height);
        }
    }
}

#[test]
fn the_source_no_longer_uses_the_reversed_modifier() {
    assert!(!include_str!("mod.rs").contains("REVERSED"));
}

fn modified(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

#[test]
fn control_and_alt_combinations_insert_nothing() {
    let mut dialog = TextInputDialog::new("New folder", "abc");
    for (character, modifiers) in [
        ('s', KeyModifiers::CONTROL),
        ('u', KeyModifiers::CONTROL),
        ('x', KeyModifiers::ALT),
        ('y', KeyModifiers::CONTROL | KeyModifiers::SHIFT),
    ] {
        dialog.handle_key(modified(KeyCode::Char(character), modifiers));
    }
    assert_eq!(dialog.value, "abc");
}

#[test]
fn control_with_alt_is_alt_gr_and_inserts_the_character() {
    let mut dialog = TextInputDialog::new("New folder", "");
    dialog.handle_key(modified(KeyCode::Char('@'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(dialog.value, "@");
}

#[test]
fn shift_still_inserts_the_character() {
    let mut dialog = TextInputDialog::new("New folder", "");
    dialog.handle_key(modified(KeyCode::Char('A'), KeyModifiers::SHIFT));
    assert_eq!(dialog.value, "A");
}

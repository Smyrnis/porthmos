use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn typed(field: &mut PathField, text: &str) {
    for character in text.chars() {
        field.handle_key(key(KeyCode::Char(character)));
    }
}

#[test]
fn a_new_field_puts_the_cursor_at_the_end() {
    let field = PathField::new("/a/b".to_string());

    assert_eq!(field.text, "/a/b");
    assert_eq!(field.cursor, 4);
}

#[test]
fn typing_inserts_at_the_cursor_and_moves_it() {
    let mut field = PathField::new("/ac".to_string());
    field.handle_key(key(KeyCode::Left));

    assert_eq!(field.handle_key(key(KeyCode::Char('b'))), PathOutcome::Pending);

    assert_eq!(field.text, "/abc");
    assert_eq!(field.cursor, 3);
}

#[test]
fn multibyte_characters_keep_the_cursor_on_character_boundaries() {
    let mut field = PathField::new(String::new());
    typed(&mut field, "α/β");

    field.handle_key(key(KeyCode::Left));
    field.handle_key(key(KeyCode::Backspace));

    assert_eq!(field.text, "αβ");
    assert_eq!(field.cursor, 1);
}

#[test]
fn backspace_at_the_start_and_delete_at_the_end_do_nothing() {
    let mut field = PathField::new("ab".to_string());
    field.handle_key(key(KeyCode::Delete));
    assert_eq!(field.text, "ab");

    field.handle_key(key(KeyCode::Home));
    field.handle_key(key(KeyCode::Backspace));
    assert_eq!(field.text, "ab");
    assert_eq!(field.cursor, 0);
}

#[test]
fn delete_removes_the_character_under_the_cursor() {
    let mut field = PathField::new("abc".to_string());
    field.handle_key(key(KeyCode::Home));

    field.handle_key(key(KeyCode::Delete));

    assert_eq!(field.text, "bc");
    assert_eq!(field.cursor, 0);
}

#[test]
fn the_cursor_stays_inside_the_text() {
    let mut field = PathField::new("ab".to_string());

    field.handle_key(key(KeyCode::Right));
    assert_eq!(field.cursor, 2);
    field.handle_key(key(KeyCode::Home));
    field.handle_key(key(KeyCode::Left));
    assert_eq!(field.cursor, 0);
    field.handle_key(key(KeyCode::End));
    assert_eq!(field.cursor, 2);
}

#[test]
fn control_and_alt_characters_are_not_inserted() {
    let mut field = PathField::new(String::new());

    for modifiers in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
        let event = KeyEvent { modifiers, ..key(KeyCode::Char('x')) };
        assert_eq!(field.handle_key(event), PathOutcome::Pending);
    }

    assert_eq!(field.text, "");
}

#[test]
fn enter_submits_and_escape_cancels() {
    let mut field = PathField::new("/a".to_string());

    assert_eq!(field.handle_key(key(KeyCode::Enter)), PathOutcome::Submit);
    assert_eq!(field.handle_key(key(KeyCode::Esc)), PathOutcome::Cancel);
    assert_eq!(field.text, "/a");
}

#[test]
fn other_keys_are_pending() {
    let mut field = PathField::new("/a".to_string());

    for code in [KeyCode::Tab, KeyCode::Up, KeyCode::F(5), KeyCode::PageDown] {
        assert_eq!(field.handle_key(key(code)), PathOutcome::Pending);
    }
}

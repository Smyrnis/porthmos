use iced::keyboard::{Key, Modifiers, key::Named};
use porthmos_term::{Key as TermKey, Modes, Modifiers as TermModifiers, encode_key};

use super::*;
use crate::shortcuts::KeyPress;

fn press(key: Key, modifiers: Modifiers, text: Option<&str>) -> KeyPress {
    KeyPress { modified_key: key.clone(), key, modifiers, text: text.map(str::to_string) }
}

fn character(text: &str) -> Key {
    Key::Character(text.into())
}

fn bytes(input: Option<KeyInput>, modes: Modes) -> Option<Vec<u8>> {
    match input? {
        KeyInput::Key(key, modifiers) => encode_key(key, modifiers, modes),
        KeyInput::Text(text) => Some(text.into_bytes()),
    }
}

#[test]
fn tab_and_ctrl_p_translate_like_any_other_key() {
    let tab = translate(&press(Key::Named(Named::Tab), Modifiers::empty(), None));
    let ctrl_p = translate(&press(character("p"), Modifiers::CTRL, Some("\u{10}")));

    assert_eq!(bytes(tab, Modes::default()), Some(b"\t".to_vec()));
    assert_eq!(bytes(ctrl_p, Modes::default()), Some(vec![0x10]));
}

#[test]
fn alt_digit_keeps_the_alt_modifier() {
    let alt_1 = translate(&press(character("1"), Modifiers::ALT, Some("1")));

    assert_eq!(bytes(alt_1, Modes::default()), Some(b"\x1b1".to_vec()));
}

#[test]
fn text_from_a_dead_key_or_ime_is_passed_through_unchanged() {
    let composed = translate(&press(Key::Unidentified, Modifiers::empty(), Some("é")));
    let committed = translate(&press(Key::Unidentified, Modifiers::empty(), Some("你好")));

    assert_eq!(composed, Some(KeyInput::Text("é".to_string())));
    assert_eq!(committed, Some(KeyInput::Text("你好".to_string())));
}

#[test]
fn shifted_symbols_use_the_typed_character() {
    let plus = translate(&press(character("="), Modifiers::SHIFT, Some("+")));

    assert_eq!(bytes(plus, Modes::default()), Some(b"+".to_vec()));
}

#[test]
fn space_and_ctrl_space_translate() {
    let space = translate(&press(Key::Named(Named::Space), Modifiers::empty(), Some(" ")));
    let nul = translate(&press(Key::Named(Named::Space), Modifiers::CTRL, None));

    assert_eq!(bytes(space, Modes::default()), Some(b" ".to_vec()));
    assert_eq!(bytes(nul, Modes::default()), Some(vec![0]));
}

#[test]
fn modifier_only_and_unmapped_presses_translate_to_nothing() {
    for named in [Named::Shift, Named::Control, Named::Alt, Named::Super, Named::CapsLock] {
        assert_eq!(translate(&press(Key::Named(named), Modifiers::empty(), None)), None, "{named:?}");
    }
    assert_eq!(translate(&press(Key::Unidentified, Modifiers::empty(), None)), None);
}

#[test]
fn super_combinations_are_left_to_the_system() {
    assert_eq!(translate(&press(character("c"), Modifiers::LOGO, Some("c"))), None);
}

#[test]
fn only_ctrl_shift_space_is_the_chord() {
    let chord = Modifiers::CTRL | Modifiers::SHIFT;

    assert!(is_chord(&press(Key::Named(Named::Space), chord, None)));
    assert!(!is_chord(&press(Key::Named(Named::Space), Modifiers::CTRL, None)));
    assert!(!is_chord(&press(Key::Named(Named::Space), Modifiers::SHIFT, None)));
    assert!(!is_chord(&press(Key::Named(Named::Space), chord | Modifiers::ALT, None)));
    assert!(!is_chord(&press(character("p"), chord, None)));
}

#[test]
fn arrows_encode_differently_in_application_cursor_mode() {
    let up = || translate(&press(Key::Named(Named::ArrowUp), Modifiers::empty(), None));
    let application = Modes { application_cursor: true, ..Modes::default() };

    assert_eq!(bytes(up(), Modes::default()), Some(b"\x1b[A".to_vec()));
    assert_eq!(bytes(up(), application), Some(b"\x1bOA".to_vec()));
}

#[test]
fn function_keys_and_shift_tab_translate() {
    let f5 = translate(&press(Key::Named(Named::F5), Modifiers::empty(), None));
    let back = translate(&press(Key::Named(Named::Tab), Modifiers::SHIFT, None));

    assert_eq!(f5, Some(KeyInput::Key(TermKey::Function(5), TermModifiers::default())));
    assert_eq!(bytes(back, Modes::default()), Some(b"\x1b[Z".to_vec()));
}

#[test]
fn altgr_characters_are_typed_as_text_not_as_ctrl_alt_chords() {
    let altgr = Modifiers::CTRL | Modifiers::ALT;

    let at = translate(&press(character("@"), altgr, Some("@")));
    let brace = translate(&press(character("{"), altgr, Some("{")));

    assert_eq!(bytes(at, Modes::default()), Some(b"@".to_vec()));
    assert_eq!(bytes(brace, Modes::default()), Some(b"{".to_vec()));
}

#[test]
fn ctrl_alt_without_printable_text_stays_a_chord() {
    let chord = translate(&press(character("c"), Modifiers::CTRL | Modifiers::ALT, Some("\u{3}")));

    assert_eq!(
        chord,
        Some(KeyInput::Key(TermKey::Char('c'), TermModifiers { shift: false, control: true, alt: true }))
    );
}

use super::*;
use crate::{Modes, MouseMode};

const NONE: Modifiers = Modifiers { shift: false, control: false, alt: false };
const CONTROL: Modifiers = Modifiers { control: true, ..NONE };
const ALT: Modifiers = Modifiers { alt: true, ..NONE };
const SHIFT: Modifiers = Modifiers { shift: true, ..NONE };

fn key(key: Key, modifiers: Modifiers) -> Option<Vec<u8>> {
    encode_key(key, modifiers, Modes::default())
}

fn application_cursor() -> Modes {
    Modes { application_cursor: true, ..Modes::default() }
}

fn mouse_modes(mouse: MouseMode, sgr: bool) -> Modes {
    Modes { mouse, mouse_sgr: sgr, ..Modes::default() }
}

fn event(action: MouseAction, column: u16, row: u16) -> MouseEvent {
    MouseEvent { action, column, row, modifiers: NONE }
}

#[test]
fn printable_and_control_characters() {
    assert_eq!(key(Key::Char('a'), NONE), Some(b"a".to_vec()));
    assert_eq!(key(Key::Char('c'), CONTROL), Some(vec![0x03]));
    assert_eq!(key(Key::Char('C'), CONTROL), Some(vec![0x03]));
    assert_eq!(key(Key::Char(' '), CONTROL), Some(vec![0x00]));
    assert_eq!(key(Key::Char('['), CONTROL), Some(vec![0x1b]));
    assert_eq!(key(Key::Char('x'), ALT), Some(b"\x1bx".to_vec()));
    assert_eq!(key(Key::Char('é'), NONE), Some("é".as_bytes().to_vec()));
    assert_eq!(key(Key::Char('é'), ALT), Some("\u{1b}é".as_bytes().to_vec()));
}

#[test]
fn editing_keys() {
    assert_eq!(key(Key::Enter, NONE), Some(b"\r".to_vec()));
    assert_eq!(key(Key::Backspace, NONE), Some(vec![0x7f]));
    assert_eq!(key(Key::Tab, NONE), Some(b"\t".to_vec()));
    assert_eq!(key(Key::Tab, SHIFT), Some(b"\x1b[Z".to_vec()));
    assert_eq!(key(Key::Escape, NONE), Some(vec![0x1b]));
    assert_eq!(key(Key::PageUp, NONE), Some(b"\x1b[5~".to_vec()));
    assert_eq!(key(Key::PageDown, NONE), Some(b"\x1b[6~".to_vec()));
    assert_eq!(key(Key::Insert, NONE), Some(b"\x1b[2~".to_vec()));
    assert_eq!(key(Key::Delete, NONE), Some(b"\x1b[3~".to_vec()));
}

#[test]
fn cursor_keys_follow_the_application_cursor_mode() {
    assert_eq!(key(Key::Up, NONE), Some(b"\x1b[A".to_vec()));
    assert_eq!(encode_key(Key::Up, NONE, application_cursor()), Some(b"\x1bOA".to_vec()));
    assert_eq!(key(Key::Home, NONE), Some(b"\x1b[H".to_vec()));
    assert_eq!(encode_key(Key::Home, NONE, application_cursor()), Some(b"\x1bOH".to_vec()));
    assert_eq!(key(Key::Left, NONE), Some(b"\x1b[D".to_vec()));
    assert_eq!(key(Key::End, NONE), Some(b"\x1b[F".to_vec()));
}

#[test]
fn modified_special_keys_use_the_modifier_parameter() {
    assert_eq!(key(Key::Up, CONTROL), Some(b"\x1b[1;5A".to_vec()));
    assert_eq!(encode_key(Key::Up, CONTROL, application_cursor()), Some(b"\x1b[1;5A".to_vec()));
    assert_eq!(key(Key::Right, Modifiers { shift: true, alt: true, control: false }), Some(b"\x1b[1;4C".to_vec()));
    assert_eq!(key(Key::Delete, CONTROL), Some(b"\x1b[3;5~".to_vec()));
}

#[test]
fn function_keys() {
    assert_eq!(key(Key::Function(1), NONE), Some(b"\x1bOP".to_vec()));
    assert_eq!(key(Key::Function(4), NONE), Some(b"\x1bOS".to_vec()));
    assert_eq!(key(Key::Function(5), NONE), Some(b"\x1b[15~".to_vec()));
    assert_eq!(key(Key::Function(12), NONE), Some(b"\x1b[24~".to_vec()));
    assert_eq!(key(Key::Function(1), SHIFT), Some(b"\x1b[1;2P".to_vec()));
    assert_eq!(key(Key::Function(0), NONE), None);
    assert_eq!(key(Key::Function(13), NONE), None);
}

#[test]
fn mouse_reporting_is_off_by_default() {
    assert_eq!(encode_mouse(event(MouseAction::Press(MouseButton::Left), 4, 2), Modes::default()), None);
}

#[test]
fn sgr_mouse_reports_press_release_and_wheel() {
    let modes = mouse_modes(MouseMode::Click, true);

    assert_eq!(encode_mouse(event(MouseAction::Press(MouseButton::Left), 4, 2), modes), Some(b"\x1b[<0;5;3M".to_vec()));
    assert_eq!(
        encode_mouse(event(MouseAction::Release(MouseButton::Left), 4, 2), modes),
        Some(b"\x1b[<0;5;3m".to_vec())
    );
    assert_eq!(
        encode_mouse(event(MouseAction::Press(MouseButton::Right), 0, 0), modes),
        Some(b"\x1b[<2;1;1M".to_vec())
    );
    assert_eq!(encode_mouse(event(MouseAction::WheelUp, 0, 0), modes), Some(b"\x1b[<64;1;1M".to_vec()));
    assert_eq!(encode_mouse(event(MouseAction::WheelDown, 0, 0), modes), Some(b"\x1b[<65;1;1M".to_vec()));
    let shifted = MouseEvent { modifiers: SHIFT, ..event(MouseAction::Press(MouseButton::Left), 0, 0) };
    assert_eq!(encode_mouse(shifted, modes), Some(b"\x1b[<4;1;1M".to_vec()));
}

#[test]
fn motion_is_reported_only_in_the_modes_that_ask_for_it() {
    let click = mouse_modes(MouseMode::Click, true);
    let drag = mouse_modes(MouseMode::Drag, true);
    let motion = mouse_modes(MouseMode::Motion, true);
    let held = event(MouseAction::Move(Some(MouseButton::Left)), 1, 1);
    let free = event(MouseAction::Move(None), 1, 1);

    assert_eq!(encode_mouse(held, click), None);
    assert_eq!(encode_mouse(held, drag), Some(b"\x1b[<32;2;2M".to_vec()));
    assert_eq!(encode_mouse(free, drag), None);
    assert_eq!(encode_mouse(free, motion), Some(b"\x1b[<35;2;2M".to_vec()));
}

#[test]
fn legacy_mouse_encoding_is_bounded() {
    let modes = mouse_modes(MouseMode::Click, false);

    assert_eq!(
        encode_mouse(event(MouseAction::Press(MouseButton::Left), 4, 2), modes),
        Some(vec![0x1b, b'[', b'M', 32, 37, 35])
    );
    assert_eq!(
        encode_mouse(event(MouseAction::Release(MouseButton::Left), 4, 2), modes),
        Some(vec![0x1b, b'[', b'M', 35, 37, 35])
    );
    assert!(encode_mouse(event(MouseAction::Press(MouseButton::Left), 222, 0), modes).is_some());
    assert_eq!(encode_mouse(event(MouseAction::Press(MouseButton::Left), 223, 0), modes), None);
    assert_eq!(encode_mouse(event(MouseAction::Press(MouseButton::Left), 0, 223), modes), None);
}

#[test]
fn mouse_coordinates_at_the_u16_limit_do_not_overflow() {
    let modes = mouse_modes(MouseMode::Click, true);

    assert_eq!(
        encode_mouse(event(MouseAction::Press(MouseButton::Left), u16::MAX, u16::MAX), modes),
        Some(b"\x1b[<0;65536;65536M".to_vec())
    );
}

#[test]
fn bracketed_paste_wraps_the_text() {
    let modes = Modes { bracketed_paste: true, ..Modes::default() };

    assert_eq!(encode_paste("ls -l\nwho", modes), b"\x1b[200~ls -l\nwho\x1b[201~".to_vec());
}

#[test]
fn pasted_text_cannot_end_the_bracketed_paste_early() {
    let modes = Modes { bracketed_paste: true, ..Modes::default() };

    let encoded = encode_paste("safe\x1b[201~rm -rf ~\n", modes);

    assert!(encoded.starts_with(b"\x1b[200~"));
    assert!(encoded.ends_with(b"\x1b[201~"));
    assert_eq!(encoded.iter().filter(|byte| **byte == 0x1b).count(), 2);
}

#[test]
fn unbracketed_paste_turns_newlines_into_returns() {
    assert_eq!(encode_paste("a\nb\r\nc", Modes::default()), b"a\rb\rc".to_vec());
}

#[test]
fn pasted_c1_controls_cannot_end_the_bracketed_paste() {
    let modes = Modes { bracketed_paste: true, ..Modes::default() };

    let encoded = encode_paste("a\u{9b}201~b\u{80}c", modes);

    assert!(!encoded.windows(2).any(|pair| pair[0] == 0xc2 && (0x80..=0x9f).contains(&pair[1])));
    assert!(String::from_utf8_lossy(&encoded).contains("a201~bc"));
}

#[test]
fn non_bracketed_paste_strips_escape_and_c1_controls() {
    assert_eq!(encode_paste("a\u{1b}[31mb\u{85}c", Modes::default()), b"a[31mbc".to_vec());
}

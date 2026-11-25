use crate::{Modes, MouseMode};

const ESCAPE: u8 = 0x1b;
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
const LEGACY_MOUSE_LIMIT: u16 = 222;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

impl Modifiers {
    fn any(self) -> bool {
        self.shift || self.control || self.alt
    }

    fn parameter(self) -> u8 {
        1 + u8::from(self.shift) + 2 * u8::from(self.alt) + 4 * u8::from(self.control)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Function(u8),
}

fn control_byte(character: char) -> Option<u8> {
    match character {
        'a'..='z' => Some(character as u8 - b'a' + 1),
        'A'..='Z' => Some(character as u8 - b'A' + 1),
        ' ' | '@' => Some(0x00),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}

fn with_alt(modifiers: Modifiers, mut bytes: Vec<u8>) -> Vec<u8> {
    if modifiers.alt {
        bytes.insert(0, ESCAPE);
    }
    bytes
}

fn csi_letter(letter: char, modifiers: Modifiers, application: bool) -> Vec<u8> {
    if modifiers.any() {
        format!("\x1b[1;{}{letter}", modifiers.parameter()).into_bytes()
    } else if application {
        format!("\x1bO{letter}").into_bytes()
    } else {
        format!("\x1b[{letter}").into_bytes()
    }
}

fn csi_tilde(code: u8, modifiers: Modifiers) -> Vec<u8> {
    if modifiers.any() {
        format!("\x1b[{code};{}~", modifiers.parameter()).into_bytes()
    } else {
        format!("\x1b[{code}~").into_bytes()
    }
}

fn function_key(number: u8, modifiers: Modifiers) -> Option<Vec<u8>> {
    match number {
        1..=4 => Some(csi_letter(['P', 'Q', 'R', 'S'][usize::from(number - 1)], modifiers, true)),
        5..=12 => {
            let code = [15, 17, 18, 19, 20, 21, 23, 24][usize::from(number - 5)];
            Some(csi_tilde(code, modifiers))
        }
        _ => None,
    }
}

pub fn encode_key(key: Key, modifiers: Modifiers, modes: Modes) -> Option<Vec<u8>> {
    let application = modes.application_cursor;
    Some(match key {
        Key::Char(character) => {
            let controlled = if modifiers.control { control_byte(character) } else { None };
            let bytes = match controlled {
                Some(byte) => vec![byte],
                None => character.to_string().into_bytes(),
            };
            with_alt(modifiers, bytes)
        }
        Key::Enter => with_alt(modifiers, vec![b'\r']),
        Key::Tab if modifiers.shift => b"\x1b[Z".to_vec(),
        Key::Tab => with_alt(modifiers, vec![b'\t']),
        Key::Backspace => with_alt(modifiers, vec![if modifiers.control { 0x08 } else { 0x7f }]),
        Key::Escape => with_alt(modifiers, vec![ESCAPE]),
        Key::Up => csi_letter('A', modifiers, application),
        Key::Down => csi_letter('B', modifiers, application),
        Key::Right => csi_letter('C', modifiers, application),
        Key::Left => csi_letter('D', modifiers, application),
        Key::Home => csi_letter('H', modifiers, application),
        Key::End => csi_letter('F', modifiers, application),
        Key::Insert => csi_tilde(2, modifiers),
        Key::Delete => csi_tilde(3, modifiers),
        Key::PageUp => csi_tilde(5, modifiers),
        Key::PageDown => csi_tilde(6, modifiers),
        Key::Function(number) => function_key(number, modifiers)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Press(MouseButton),
    Release(MouseButton),
    Move(Option<MouseButton>),
    WheelUp,
    WheelDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    pub action: MouseAction,
    pub column: u16,
    pub row: u16,
    pub modifiers: Modifiers,
}

fn button_code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

fn is_reported(action: MouseAction, mode: MouseMode) -> bool {
    match action {
        MouseAction::Move(None) => mode == MouseMode::Motion,
        MouseAction::Move(Some(_)) => matches!(mode, MouseMode::Drag | MouseMode::Motion),
        _ => mode != MouseMode::Off,
    }
}

pub fn encode_mouse(event: MouseEvent, modes: Modes) -> Option<Vec<u8>> {
    if !is_reported(event.action, modes.mouse) {
        return None;
    }
    let (base, released) = match event.action {
        MouseAction::Press(button) => (button_code(button), false),
        MouseAction::Release(button) => (if modes.mouse_sgr { button_code(button) } else { 3 }, true),
        MouseAction::Move(Some(button)) => (32 + button_code(button), false),
        MouseAction::Move(None) => (35, false),
        MouseAction::WheelUp => (64, false),
        MouseAction::WheelDown => (65, false),
    };
    let modifiers = 4 * u8::from(event.modifiers.shift)
        + 8 * u8::from(event.modifiers.alt)
        + 16 * u8::from(event.modifiers.control);
    let code = base + modifiers;
    if modes.mouse_sgr {
        let final_byte = if released { 'm' } else { 'M' };
        return Some(
            format!("\x1b[<{code};{};{}{final_byte}", u32::from(event.column) + 1, u32::from(event.row) + 1)
                .into_bytes(),
        );
    }
    if event.column > LEGACY_MOUSE_LIMIT || event.row > LEGACY_MOUSE_LIMIT {
        return None;
    }
    Some(vec![ESCAPE, b'[', b'M', 32 + code, 33 + event.column as u8, 33 + event.row as u8])
}

pub fn encode_paste(text: &str, modes: Modes) -> Vec<u8> {
    if modes.bracketed_paste {
        let mut encoded = PASTE_START.to_vec();
        let controls = |character: &char| *character == char::from(ESCAPE) || ('\u{80}'..='\u{9f}').contains(character);
        encoded.extend(text.chars().filter(|character| !controls(character)).collect::<String>().bytes());
        encoded.extend_from_slice(PASTE_END);
        return encoded;
    }
    text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
}

#[cfg(test)]
mod tests;

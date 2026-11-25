use iced::keyboard::{Key, key::Named};
use porthmos_term::{Key as TermKey, Modifiers as TermModifiers};

use crate::shortcuts::KeyPress;

const FIRST_FUNCTION_KEY: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum KeyInput {
    Key(TermKey, TermModifiers),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalChord {
    Focus,
    Copy,
    Paste,
    Search,
    PageUp,
    PageDown,
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
}

pub fn chord_of(press: &KeyPress) -> Option<TerminalChord> {
    let modifiers = press.modifiers;
    if modifiers.alt() || modifiers.logo() {
        return None;
    }
    if modifiers.control() && modifiers.shift() {
        return match &press.key {
            Key::Named(Named::Space) => Some(TerminalChord::Focus),
            Key::Named(Named::PageDown) => Some(TerminalChord::NextTab),
            Key::Named(Named::PageUp) => Some(TerminalChord::PreviousTab),
            Key::Character(text) => match text.to_ascii_lowercase().as_str() {
                "c" => Some(TerminalChord::Copy),
                "v" => Some(TerminalChord::Paste),
                "f" => Some(TerminalChord::Search),
                "t" => Some(TerminalChord::NewTab),
                "w" => Some(TerminalChord::CloseTab),
                _ => None,
            },
            _ => None,
        };
    }
    if modifiers.shift() && !modifiers.control() {
        return match &press.key {
            Key::Named(Named::PageUp) => Some(TerminalChord::PageUp),
            Key::Named(Named::PageDown) => Some(TerminalChord::PageDown),
            _ => None,
        };
    }
    None
}

fn named_key(named: Named) -> Option<TermKey> {
    Some(match named {
        Named::Enter => TermKey::Enter,
        Named::Tab => TermKey::Tab,
        Named::Backspace => TermKey::Backspace,
        Named::Escape => TermKey::Escape,
        Named::ArrowUp => TermKey::Up,
        Named::ArrowDown => TermKey::Down,
        Named::ArrowLeft => TermKey::Left,
        Named::ArrowRight => TermKey::Right,
        Named::Home => TermKey::Home,
        Named::End => TermKey::End,
        Named::PageUp => TermKey::PageUp,
        Named::PageDown => TermKey::PageDown,
        Named::Insert => TermKey::Insert,
        Named::Delete => TermKey::Delete,
        Named::Space => TermKey::Char(' '),
        Named::F1 => function(0),
        Named::F2 => function(1),
        Named::F3 => function(2),
        Named::F4 => function(3),
        Named::F5 => function(4),
        Named::F6 => function(5),
        Named::F7 => function(6),
        Named::F8 => function(7),
        Named::F9 => function(8),
        Named::F10 => function(9),
        Named::F11 => function(10),
        Named::F12 => function(11),
        _ => return None,
    })
}

fn function(index: u8) -> TermKey {
    TermKey::Function(FIRST_FUNCTION_KEY + index)
}

fn typed_text(press: &KeyPress) -> Option<&str> {
    press.text.as_deref().filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
}

fn first_character(key: &Key) -> Option<char> {
    match key {
        Key::Character(text) => text.chars().next(),
        _ => None,
    }
}

pub fn translate(press: &KeyPress) -> Option<KeyInput> {
    if press.modifiers.logo() {
        return None;
    }
    let modifiers = TermModifiers {
        shift: press.modifiers.shift(),
        control: press.modifiers.control(),
        alt: press.modifiers.alt(),
    };
    match &press.key {
        Key::Named(named) => named_key(*named).map(|key| KeyInput::Key(key, modifiers)),
        Key::Character(_) if modifiers.control && modifiers.alt && typed_text(press).is_some() => {
            typed_text(press).map(|text| KeyInput::Text(text.to_string()))
        }
        Key::Character(_) if modifiers.control => {
            first_character(&press.key).map(|character| KeyInput::Key(TermKey::Char(character), modifiers))
        }
        Key::Character(_) if modifiers.alt => first_character(&press.modified_key)
            .or_else(|| first_character(&press.key))
            .map(|character| KeyInput::Key(TermKey::Char(character), modifiers)),
        Key::Character(_) => match typed_text(press) {
            Some(text) if text.chars().count() > 1 => Some(KeyInput::Text(text.to_string())),
            Some(text) => text.chars().next().map(|character| KeyInput::Key(TermKey::Char(character), modifiers)),
            None => first_character(&press.key).map(|character| KeyInput::Key(TermKey::Char(character), modifiers)),
        },
        Key::Unidentified => typed_text(press).map(|text| KeyInput::Text(text.to_string())),
    }
}

#[cfg(test)]
mod tests;

use super::{LogLine, MAX_LOG_LINE_BYTES, detect_level};
use crate::hides_or_reorders_text;

const MARKER: char = '…';
const TAB_SPACES: &str = "    ";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum State {
    #[default]
    Text,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

#[derive(Debug, Default)]
pub struct LineSplitter {
    raw: Vec<u8>,
    overflowed: bool,
    state: State,
}

impl LineSplitter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<LogLine>) {
        for &byte in bytes {
            if byte == b'\n' {
                self.state = State::Text;
                self.emit(out);
            } else {
                self.step(byte);
            }
        }
    }

    pub fn finish(&mut self, out: &mut Vec<LogLine>) {
        self.state = State::Text;
        if !self.raw.is_empty() || self.overflowed {
            self.emit(out);
        }
    }

    #[cfg(test)]
    pub(super) fn buffered_capacity(&self) -> usize {
        self.raw.capacity()
    }

    fn step(&mut self, byte: u8) {
        self.state = match (self.state, byte) {
            (State::Text, 0x1b) => State::Escape,
            (State::Text, _) => {
                self.push(byte);
                State::Text
            }
            (State::Escape, b'[') => State::Csi,
            (State::Escape, b']') => State::Osc,
            (State::Escape, 0x1b | 0x20..=0x2f) => State::Escape,
            (State::Escape, _) => State::Text,
            (State::Csi, 0x40..=0x7e) => State::Text,
            (State::Csi, _) => State::Csi,
            (State::Osc, 0x07) | (State::OscEscape, b'\\') => State::Text,
            (State::Osc, 0x1b) => State::OscEscape,
            (State::Osc | State::OscEscape, _) => State::Osc,
        };
    }

    fn push(&mut self, byte: u8) {
        if self.raw.len() < MAX_LOG_LINE_BYTES {
            self.raw.push(byte);
        } else if byte == b'\t' || !byte.is_ascii_control() {
            self.overflowed = true;
        }
    }

    fn emit(&mut self, out: &mut Vec<LogLine>) {
        let mut raw = std::mem::take(&mut self.raw);
        let mut truncated = std::mem::take(&mut self.overflowed);
        if truncated
            && let Err(error) = std::str::from_utf8(&raw)
            && error.error_len().is_none()
        {
            raw.truncate(error.valid_up_to());
        }
        let mut text = clean(&String::from_utf8_lossy(&raw));
        truncated |= text.len() > MAX_LOG_LINE_BYTES;
        if truncated {
            let mut end = text.len().min(MAX_LOG_LINE_BYTES - MARKER.len_utf8());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text.push(MARKER);
        }
        let level = detect_level(&text);
        out.push(LogLine { text, level });
    }
}

fn clean(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    for character in text.chars() {
        if character == '\t' {
            cleaned.push_str(TAB_SPACES);
        } else if !character.is_control() && !hides_or_reorders_text(character) {
            cleaned.push(character);
        }
    }
    cleaned
}

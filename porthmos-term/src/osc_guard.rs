use std::borrow::Cow;

const ESCAPE: u8 = 0x1b;
const BELL: u8 = 0x07;
const CANCEL: u8 = 0x18;
const SUBSTITUTE: u8 = 0x1a;
const STRING_TERMINATOR: u8 = b'\\';
const OSC_INTRODUCER: u8 = b']';
pub(crate) const MAX_OSC_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    String,
    StringEscape,
    Discarding,
}

pub(crate) struct OscGuard {
    state: State,
    held: Vec<u8>,
}

fn ends_string(byte: u8) -> bool {
    matches!(byte, BELL | CANCEL | SUBSTITUTE)
}

impl OscGuard {
    pub(crate) fn new() -> Self {
        Self { state: State::Ground, held: Vec::new() }
    }

    pub(crate) fn filter<'a>(&mut self, bytes: &'a [u8]) -> Cow<'a, [u8]> {
        if self.state == State::Ground && !bytes.contains(&ESCAPE) {
            return Cow::Borrowed(bytes);
        }
        let mut output = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            if self.state == State::Ground {
                let end = bytes[index..].iter().position(|byte| *byte == ESCAPE).map_or(bytes.len(), |at| index + at);
                output.extend_from_slice(&bytes[index..end]);
                index = end;
                if index == bytes.len() {
                    break;
                }
            }
            self.advance(bytes[index], &mut output);
            index += 1;
        }
        Cow::Owned(output)
    }

    fn release(&mut self, output: &mut Vec<u8>) {
        output.append(&mut self.held);
    }

    fn start_escape(&mut self) {
        self.held.clear();
        self.held.push(ESCAPE);
        self.state = State::Escape;
    }

    fn advance(&mut self, byte: u8, output: &mut Vec<u8>) {
        match self.state {
            State::Ground => {
                if byte == ESCAPE {
                    self.start_escape();
                } else {
                    output.push(byte);
                }
            }
            State::Escape => match byte {
                OSC_INTRODUCER => {
                    self.held.push(byte);
                    self.state = State::String;
                }
                ESCAPE => {
                    self.release(output);
                    self.start_escape();
                }
                _ => {
                    self.release(output);
                    output.push(byte);
                    self.state = State::Ground;
                }
            },
            State::String => {
                if ends_string(byte) {
                    self.release(output);
                    output.push(byte);
                    self.state = State::Ground;
                } else if byte == ESCAPE {
                    self.held.push(byte);
                    self.state = State::StringEscape;
                } else if self.held.len() >= MAX_OSC_BYTES {
                    self.held.clear();
                    self.state = State::Discarding;
                } else {
                    self.held.push(byte);
                }
            }
            State::StringEscape => {
                if byte == STRING_TERMINATOR {
                    self.held.push(byte);
                    self.release(output);
                    self.state = State::Ground;
                } else {
                    self.held.pop();
                    self.release(output);
                    self.start_escape();
                    self.advance(byte, output);
                }
            }
            State::Discarding => {
                if ends_string(byte) {
                    self.state = State::Ground;
                } else if byte == ESCAPE {
                    self.start_escape();
                }
            }
        }
    }
}

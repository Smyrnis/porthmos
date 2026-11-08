use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathField {
    pub text: String,
    pub cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOutcome {
    Pending,
    Submit,
    Cancel,
}

impl PathField {
    pub fn new(text: String) -> Self {
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> PathOutcome {
        match key.code {
            KeyCode::Enter => return PathOutcome::Submit,
            KeyCode::Esc => return PathOutcome::Cancel,
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.length()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.length(),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.text.remove(self.byte_index(self.cursor));
            }
            KeyCode::Delete if self.cursor < self.length() => {
                self.text.remove(self.byte_index(self.cursor));
            }
            KeyCode::Char(character) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                self.text.insert(self.byte_index(self.cursor), character);
                self.cursor += 1;
            }
            _ => {}
        }
        PathOutcome::Pending
    }

    fn length(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index(&self, character_index: usize) -> usize {
        self.text.char_indices().nth(character_index).map_or(self.text.len(), |(byte, _)| byte)
    }
}

#[cfg(test)]
mod tests;

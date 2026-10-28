use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect};

use crate::{
    theme::Theme,
    widgets::{
        dialog::types_text,
        form_fields::render_underline_value,
        history_view::printable,
        popup::{content_width, footer, footer_text, render_popup},
    },
};

const MIN_WIDTH: u16 = 40;
const HEIGHT: u16 = 6;

pub struct TextInputDialog {
    pub title: String,
    pub value: String,
    pub cursor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputOutcome {
    Pending,
    Submitted(String),
    Cancelled,
}

impl TextInputDialog {
    pub fn new(title: impl Into<String>, initial_value: impl Into<String>) -> Self {
        let value = initial_value.into();
        let cursor = value.chars().count();
        Self { title: title.into(), value, cursor }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> TextInputOutcome {
        match key.code {
            KeyCode::Enter => TextInputOutcome::Submitted(self.value.clone()),
            KeyCode::Esc => TextInputOutcome::Cancelled,
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                TextInputOutcome::Pending
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.value.chars().count());
                TextInputOutcome::Pending
            }
            KeyCode::Home => {
                self.cursor = 0;
                TextInputOutcome::Pending
            }
            KeyCode::End => {
                self.cursor = self.value.chars().count();
                TextInputOutcome::Pending
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.remove_char_at(self.cursor - 1);
                    self.cursor -= 1;
                }
                TextInputOutcome::Pending
            }
            KeyCode::Delete => {
                if self.cursor < self.value.chars().count() {
                    self.remove_char_at(self.cursor);
                }
                TextInputOutcome::Pending
            }
            KeyCode::Char(c) if types_text(key.modifiers) => {
                self.insert_char_at(self.cursor, c);
                self.cursor += 1;
                TextInputOutcome::Pending
            }
            _ => TextInputOutcome::Pending,
        }
    }

    fn insert_char_at(&mut self, index: usize, ch: char) {
        let byte_index = self.byte_index_for(index);
        self.value.insert(byte_index, ch);
    }

    fn remove_char_at(&mut self, index: usize) {
        let byte_index = self.byte_index_for(index);
        self.value.remove(byte_index);
    }

    fn byte_index_for(&self, char_index: usize) -> usize {
        self.value.char_indices().nth(char_index).map(|(byte, _)| byte).unwrap_or(self.value.len())
    }
}

pub fn render_text_input(frame: &mut Frame, area: Rect, dialog: &TextInputDialog, theme: &Theme) {
    let title = printable(&dialog.title);
    let value = printable(&dialog.value);
    let pairs = [("enter", "ok"), ("esc", "cancel")];
    let footer_line = footer_text(&pairs);
    let width = content_width(&[title.as_str(), value.as_str(), footer_line.as_str()]).max(MIN_WIDTH).min(area.width);
    let inner = render_popup(frame, area, (width, HEIGHT.min(area.height)), Some(&title), theme);
    if inner.width == 0 || inner.height < 2 {
        return;
    }
    render_underline_value(
        frame,
        Rect::new(inner.x, inner.y, inner.width, 1),
        &value,
        Some(dialog.cursor),
        true,
        theme,
    );
    if inner.height >= 4 {
        footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), &pairs, theme);
    }
}

#[cfg(test)]
mod tests;

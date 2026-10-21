use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        popup::{content_width, footer, render_popup, wrap_line},
    },
};

pub struct MessageDialog {
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageOutcome {
    Pending,
    Closed,
}

impl MessageDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { title: title.into(), message: message.into() }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> MessageOutcome {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => MessageOutcome::Closed,
            _ => MessageOutcome::Pending,
        }
    }
}

pub fn render_message(frame: &mut Frame, area: Rect, dialog: &MessageDialog, theme: &Theme) {
    let title = printable(&dialog.title);
    let lines: Vec<String> = dialog.message.lines().map(printable).collect();
    let longest = lines.iter().map(|line| line.width()).max().unwrap_or(0).min(200) as u16;
    let width = content_width(&[title.as_str(), "enter close"]).max(longest + 4).clamp(50, 76).min(area.width);
    let rows: Vec<String> =
        lines.iter().flat_map(|line| wrap_line(line, usize::from(width.saturating_sub(4)))).collect();
    let height = u16::try_from(rows.len() + 4).unwrap_or(u16::MAX).min(area.height);
    let inner = render_popup(frame, area, (width, height), Some(&title), theme);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let text: Vec<Line> = rows.into_iter().map(|row| Line::styled(row, theme.text())).collect();
    let text_area = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(2));
    frame.render_widget(Paragraph::new(text), text_area);
    footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), &[("enter", "close")], theme);
}

#[cfg(test)]
mod tests;

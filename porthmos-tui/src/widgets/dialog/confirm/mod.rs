use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        popup::{content_width, option_spans, render_popup, wrap_line},
    },
};

const OPTIONS_TEXT: &str = "  y yes     n no";

pub struct ConfirmDialog {
    pub message: String,
    pub focus: ConfirmFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmFocus {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOutcome {
    Pending,
    Confirmed,
    Cancelled,
}

impl ConfirmDialog {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), focus: ConfirmFocus::No }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ConfirmOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => ConfirmOutcome::Confirmed,
            KeyCode::Char('n') | KeyCode::Char('N') => ConfirmOutcome::Cancelled,
            KeyCode::Esc => ConfirmOutcome::Cancelled,
            KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                self.focus = match self.focus {
                    ConfirmFocus::Yes => ConfirmFocus::No,
                    ConfirmFocus::No => ConfirmFocus::Yes,
                };
                ConfirmOutcome::Pending
            }
            KeyCode::Enter => match self.focus {
                ConfirmFocus::Yes => ConfirmOutcome::Confirmed,
                ConfirmFocus::No => ConfirmOutcome::Cancelled,
            },
            _ => ConfirmOutcome::Pending,
        }
    }
}

pub fn render_confirm(frame: &mut Frame, area: Rect, dialog: &ConfirmDialog, theme: &Theme) {
    let lines: Vec<String> = dialog.message.lines().map(printable).collect();
    let mut measured: Vec<&str> = lines.iter().map(String::as_str).collect();
    measured.push(OPTIONS_TEXT);
    let width = content_width(&measured).min(area.width);
    let rows: Vec<String> =
        lines.iter().flat_map(|line| wrap_line(line, usize::from(width.saturating_sub(4)))).collect();
    let height = u16::try_from(rows.len() + 4).unwrap_or(u16::MAX).min(area.height);
    let inner = render_popup(frame, area, (width, height), None, theme);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let text: Vec<Line> = rows.into_iter().map(|row| Line::styled(row, theme.text())).collect();
    let text_area = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(2));
    frame.render_widget(Paragraph::new(text), text_area);

    let mut spans = option_spans("y", "yes", dialog.focus == ConfirmFocus::Yes, theme);
    spans.push(Span::raw("   "));
    spans.extend(option_spans("n", "no", dialog.focus == ConfirmFocus::No, theme));
    frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(inner.x, inner.bottom() - 1, inner.width, 1));
}

#[cfg(test)]
mod tests;

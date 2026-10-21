use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        popup::{content_width, footer, footer_text, render_popup},
        text_fit::truncate_with_ellipsis,
    },
};

pub struct ListDialog {
    pub title: String,
    pub items: Vec<String>,
    pub cursor: usize,
    pub removable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOutcome {
    Pending,
    Selected(usize),
    Removed(usize),
    Cancelled,
}

impl ListDialog {
    pub fn new(title: impl Into<String>, items: Vec<String>) -> Self {
        Self { title: title.into(), items, cursor: 0, removable: false }
    }

    pub fn removable(mut self, removable: bool) -> Self {
        self.removable = removable;
        self
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ListOutcome {
        match key.code {
            KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                ListOutcome::Pending
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.items.len() {
                    self.cursor += 1;
                }
                ListOutcome::Pending
            }
            KeyCode::Enter if !self.items.is_empty() => ListOutcome::Selected(self.cursor),
            KeyCode::F(8) if self.removable && !self.items.is_empty() => ListOutcome::Removed(self.cursor),
            KeyCode::Esc => ListOutcome::Cancelled,
            _ => ListOutcome::Pending,
        }
    }
}

pub fn render_list(frame: &mut Frame, area: Rect, dialog: &ListDialog, theme: &Theme) {
    let title = printable(&dialog.title);
    let items: Vec<String> = dialog.items.iter().map(|item| printable(item)).collect();
    let mut pairs = vec![("enter", "select"), ("esc", "cancel")];
    if dialog.removable {
        pairs.push(("f8", "remove"));
    }
    let footer_line = footer_text(&pairs);
    let mut measured: Vec<String> = items.iter().map(|item| format!("  {item}")).collect();
    measured.push(title.clone());
    measured.push(footer_line);
    let measured: Vec<&str> = measured.iter().map(String::as_str).collect();
    let width = content_width(&measured).min(area.width);
    let rows = items.len().max(1);
    let height = u16::try_from(rows + 4).unwrap_or(u16::MAX).min(area.height);
    let inner = render_popup(frame, area, (width, height), Some(&title), theme);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let list_items: Vec<ListItem> = if items.is_empty() {
        vec![ListItem::new(Line::styled("(empty)", theme.dim()))]
    } else {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let text = truncate_with_ellipsis(item, usize::from(inner.width).saturating_sub(2));
                let (marker, style) = if index == dialog.cursor {
                    (Span::styled("\u{203a} ", theme.accent_text()), theme.focused_name())
                } else {
                    (Span::raw("  "), theme.text())
                };
                ListItem::new(Line::from(vec![marker, Span::styled(text, style)]))
            })
            .collect()
    };
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(dialog.cursor));
    }
    let list = List::new(list_items).highlight_style(theme.focus_row());
    let list_area = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(2));
    frame.render_stateful_widget(list, list_area, &mut state);
    footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), &pairs, theme);
}

#[cfg(test)]
mod tests;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::Entry;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        form_fields::render_caret_value,
        frame::{accent_underline, hairline},
        history_view::printable,
        text_fit::{truncate_start_with_ellipsis, truncate_with_ellipsis},
    },
};

const MARKER_WIDTH: usize = 2;
const COUNT_GAP: usize = 3;
const HEADER_ROWS: u16 = 2;

pub struct SearchView {
    pub pattern: String,
    pub cursor: usize,
    pub results: Vec<Entry>,
    pub selected: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOutcome {
    Pending,
    PatternChanged,
    Open,
    Cancel,
}

impl SearchView {
    pub fn new() -> Self {
        Self { pattern: String::new(), cursor: 0, results: Vec::new(), selected: 0, truncated: false }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SearchOutcome {
        match key.code {
            KeyCode::Esc => SearchOutcome::Cancel,
            KeyCode::Enter => SearchOutcome::Open,
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                SearchOutcome::Pending
            }
            KeyCode::Down => {
                if self.selected + 1 < self.results.len() {
                    self.selected += 1;
                }
                SearchOutcome::Pending
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                SearchOutcome::Pending
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.pattern.chars().count());
                SearchOutcome::Pending
            }
            KeyCode::Home => {
                self.cursor = 0;
                SearchOutcome::Pending
            }
            KeyCode::End => {
                self.cursor = self.pattern.chars().count();
                SearchOutcome::Pending
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.remove_char_at(self.cursor - 1);
                self.cursor -= 1;
                SearchOutcome::PatternChanged
            }
            KeyCode::Delete if self.cursor < self.pattern.chars().count() => {
                self.remove_char_at(self.cursor);
                SearchOutcome::PatternChanged
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_char_at(self.cursor, c);
                self.cursor += 1;
                SearchOutcome::PatternChanged
            }
            _ => SearchOutcome::Pending,
        }
    }

    pub fn start(&mut self) {
        self.results.clear();
        self.selected = 0;
        self.truncated = false;
    }

    pub fn push_result(&mut self, entry: Entry) {
        self.results.push(entry);
    }

    pub fn finish(&mut self, truncated: bool) {
        self.truncated = truncated;
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.results.get(self.selected)
    }

    fn insert_char_at(&mut self, index: usize, ch: char) {
        let byte_index = self.byte_index_for(index);
        self.pattern.insert(byte_index, ch);
    }

    fn remove_char_at(&mut self, index: usize) {
        let byte_index = self.byte_index_for(index);
        self.pattern.remove(byte_index);
    }

    fn byte_index_for(&self, char_index: usize) -> usize {
        self.pattern.char_indices().nth(char_index).map(|(byte, _)| byte).unwrap_or(self.pattern.len())
    }
}

impl Default for SearchView {
    fn default() -> Self {
        Self::new()
    }
}

fn result_line(entry: &Entry, focused: bool, width: usize, theme: &Theme) -> Line<'static> {
    let marker = if focused { Span::styled("\u{203a} ", theme.accent_text()) } else { Span::raw("  ") };
    let budget = width.saturating_sub(MARKER_WIDTH);
    let full = printable(&entry.path.to_string());
    let split = full.rfind('/').map_or(0, |index| index + 1);
    let (directory, name) = full.split_at(split);
    let (directory, name) = if name.width() >= budget {
        (String::new(), truncate_with_ellipsis(name, budget))
    } else {
        (truncate_start_with_ellipsis(directory, budget - name.width()), name.to_string())
    };
    let name_style = if focused { theme.focused_name() } else { theme.text() };
    Line::from(vec![marker, Span::styled(directory, theme.dim()), Span::styled(name, name_style)])
}

pub fn render_search(frame: &mut Frame, area: Rect, view: &SearchView, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let count = if view.truncated { format!("{}+", view.results.len()) } else { view.results.len().to_string() };
    let count_width = count.width();
    let show_count = width >= MARKER_WIDTH + 6 + COUNT_GAP + count_width;
    frame.render_widget(
        Paragraph::new(Span::styled("/", theme.accent_text())),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let reserved = if show_count { count_width + COUNT_GAP } else { 0 };
    let pattern_width = width.saturating_sub(MARKER_WIDTH + reserved);
    if pattern_width > 0 {
        let pattern_area = Rect::new(area.x + MARKER_WIDTH as u16, area.y, pattern_width as u16, 1);
        render_caret_value(frame, pattern_area, &printable(&view.pattern), Some(view.cursor), theme.bright());
    }
    if show_count {
        let x = area.x + area.width - count_width as u16;
        frame.render_widget(
            Paragraph::new(Span::styled(count, theme.dim())),
            Rect::new(x, area.y, count_width as u16, 1),
        );
    }
    if area.height < HEADER_ROWS {
        return;
    }

    let rule = Rect::new(area.x, area.y + 1, area.width, 1);
    hairline(frame, rule, theme);
    accent_underline(frame, rule, 0..area.width, theme);
    if area.height == HEADER_ROWS {
        return;
    }

    let body = Rect::new(area.x, area.y + HEADER_ROWS, area.width, area.height - HEADER_ROWS);
    if view.results.is_empty() {
        let message = if view.pattern.is_empty() { "type to search" } else { "no matches" };
        frame.render_widget(
            Paragraph::new(Span::styled(message, theme.dim())),
            Rect::new(body.x, body.y, body.width, 1),
        );
        return;
    }
    let items: Vec<ListItem> = view
        .results
        .iter()
        .enumerate()
        .map(|(index, entry)| ListItem::new(result_line(entry, index == view.selected, width, theme)))
        .collect();
    let list = List::new(items).highlight_style(theme.focus_row());
    let mut state = ListState::default();
    state.select(Some(view.selected));
    frame.render_stateful_widget(list, body, &mut state);
}

#[cfg(test)]
mod tests;

use chrono::Local;
use porthmos_core::{
    history::{self, HistoryEntry, HistoryResult},
    transfer::Direction,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        file_list::format_size,
        filter_line::{FilterLine, FilterLineState, render_filter_line},
        frame::{accent_underline, hairline},
        text_fit::{pad_left, pad_right, truncate_with_ellipsis},
    },
};

const MARKER_WIDTH: usize = 2;
const ARROW_WIDTH: usize = 2;
const GAP: usize = 2;
const TIME_WIDTH: usize = 16;
const MAX_CONNECTION_WIDTH: usize = 18;
const MIN_ITEM_WIDTH: usize = 12;
const HEADER_ROWS: u16 = 3;

#[derive(Default)]
pub struct HistoryView {
    entries: Vec<HistoryEntry>,
    visible: Vec<usize>,
    filter: Option<String>,
    editing_filter: bool,
    cursor: usize,
}

impl HistoryView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace(&mut self, entries: Vec<HistoryEntry>) {
        let anchor = self.selected().cloned();
        self.entries = entries;
        self.refilter();
        let position = anchor.and_then(|anchor| self.rows().position(|entry| *entry == anchor));
        if let Some(position) = position {
            self.cursor = position;
        }
    }

    pub fn total(&self) -> usize {
        self.entries.len()
    }

    pub fn matched(&self) -> usize {
        self.visible.len()
    }

    pub fn summary(&self) -> String {
        match (&self.filter, self.total()) {
            (Some(_), total) => format!("{} of {total}", self.matched()),
            (None, 1) => "1 entry".to_string(),
            (None, total) => format!("{total} entries"),
        }
    }

    pub fn rows(&self) -> impl Iterator<Item = &HistoryEntry> + '_ {
        self.visible.iter().map(|index| &self.entries[*index])
    }

    pub fn selected(&self) -> Option<&HistoryEntry> {
        self.visible.get(self.cursor).map(|index| &self.entries[*index])
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn go_to_top(&mut self) {
        self.cursor = 0;
    }

    fn refilter(&mut self) {
        let filter = self.filter.as_deref();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| filter.is_none_or(|text| history::matches(entry, text)))
            .map(|(index, _)| index)
            .collect();
        self.cursor = self.cursor.min(self.visible.len().saturating_sub(1));
    }

    fn set_filter(&mut self, text: Option<String>) {
        self.filter = text.filter(|text| !text.is_empty());
        self.refilter();
    }
}

impl FilterLine for HistoryView {
    fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    fn editing_filter(&self) -> bool {
        self.editing_filter
    }

    fn start_filter(&mut self) {
        self.editing_filter = true;
    }

    fn finish_filter(&mut self) {
        self.editing_filter = false;
    }

    fn type_filter(&mut self, character: char) {
        let mut text = self.filter.clone().unwrap_or_default();
        text.push(character);
        self.set_filter(Some(text));
    }

    fn erase_filter(&mut self) {
        let mut text = self.filter.clone().unwrap_or_default();
        text.pop();
        self.set_filter(Some(text));
    }

    fn clear_filter(&mut self) {
        self.editing_filter = false;
        self.set_filter(None);
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let last = self.visible.len() as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, last) as usize;
    }
}

fn is_unprintable(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{61c}'
        )
}

pub fn printable(text: &str) -> String {
    text.chars().map(|character| if is_unprintable(character) { '?' } else { character }).collect()
}

fn status_of(result: &HistoryResult, theme: &Theme) -> (String, Style) {
    match result {
        HistoryResult::Done => ("done".to_string(), theme.ok_text()),
        HistoryResult::Failed => ("failed".to_string(), theme.err_text()),
        HistoryResult::PartlyFailed { failed } => (format!("done, {failed} failed"), theme.err_text()),
        HistoryResult::Cancelled => ("cancelled".to_string(), theme.dim()),
        HistoryResult::Interrupted => ("interrupted".to_string(), theme.dim()),
    }
}

fn time_text(entry: &HistoryEntry) -> String {
    entry.finished_at.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

fn files_text(entry: &HistoryEntry) -> String {
    format!("{}/{}", entry.files_done, entry.files_total)
}

struct Columns {
    item: usize,
    time: bool,
    connection: Option<usize>,
    files: Option<usize>,
    size: Option<usize>,
    status: usize,
}

fn columns_for(view: &HistoryView, theme: &Theme, width: usize) -> Columns {
    let widest = |texts: &mut dyn Iterator<Item = usize>, header: &str| texts.max().unwrap_or(0).max(header.len());
    let connection = widest(&mut view.rows().map(|entry| printable(&entry.connection).width()), "connection")
        .min(MAX_CONNECTION_WIDTH);
    let files = widest(&mut view.rows().map(|entry| files_text(entry).width()), "files");
    let size = widest(&mut view.rows().map(|entry| format_size(entry.bytes, false).width()), "size");
    let status = widest(&mut view.rows().map(|entry| status_of(&entry.result, theme).0.width()), "status");
    let base = MARKER_WIDTH + ARROW_WIDTH + GAP + status;

    for dropped in 0..=4usize {
        let fixed = base
            + if dropped < 4 { TIME_WIDTH + GAP } else { 0 }
            + if dropped < 3 { GAP + connection } else { 0 }
            + if dropped < 2 { GAP + files } else { 0 }
            + if dropped < 1 { GAP + size } else { 0 };
        if width >= fixed + MIN_ITEM_WIDTH || dropped == 4 {
            return Columns {
                item: width.saturating_sub(fixed).max(1),
                time: dropped < 4,
                connection: (dropped < 3).then_some(connection),
                files: (dropped < 2).then_some(files),
                size: (dropped < 1).then_some(size),
                status,
            };
        }
    }
    unreachable!("the loop returns at its last level")
}

fn header_line(columns: &Columns, theme: &Theme) -> Line<'static> {
    let gap = " ".repeat(GAP);
    let mut text = " ".repeat(MARKER_WIDTH);
    if columns.time {
        text.push_str(&format!("{}{gap}", pad_right("time", TIME_WIDTH)));
    }
    text.push_str(&" ".repeat(ARROW_WIDTH));
    text.push_str(&pad_right("item", columns.item));
    if let Some(width) = columns.connection {
        text.push_str(&format!("{gap}{}", pad_right("connection", width)));
    }
    if let Some(width) = columns.files {
        text.push_str(&format!("{gap}{}", pad_left("files", width)));
    }
    if let Some(width) = columns.size {
        text.push_str(&format!("{gap}{}", pad_left("size", width)));
    }
    text.push_str(&format!("{gap}status"));
    Line::from(Span::styled(text, theme.dim()))
}

fn row_line(entry: &HistoryEntry, focused: bool, columns: &Columns, theme: &Theme) -> Line<'static> {
    let gap = " ".repeat(GAP);
    let marker = if focused { Span::styled("\u{203a} ", theme.accent_text()) } else { Span::raw("  ") };
    let mut spans = vec![marker];
    if columns.time {
        spans.push(Span::styled(format!("{}{gap}", time_text(entry)), theme.dim()));
    }
    let arrow = match entry.direction {
        Direction::Upload => "\u{2191} ",
        Direction::Download => "\u{2193} ",
    };
    spans.push(Span::styled(arrow, theme.muted()));
    let name_style = if focused { theme.focused_name() } else { theme.text() };
    let name = pad_right(&truncate_with_ellipsis(&printable(&entry.label), columns.item), columns.item);
    spans.push(Span::styled(name, name_style));
    if let Some(width) = columns.connection {
        let connection = pad_right(&truncate_with_ellipsis(&printable(&entry.connection), width), width);
        spans.push(Span::styled(format!("{gap}{connection}"), theme.dim()));
    }
    if let Some(width) = columns.files {
        spans.push(Span::styled(format!("{gap}{}", pad_left(&files_text(entry), width)), theme.dim()));
    }
    if let Some(width) = columns.size {
        spans.push(Span::styled(format!("{gap}{}", pad_left(&format_size(entry.bytes, false), width)), theme.dim()));
    }
    let (status, status_style) = status_of(&entry.result, theme);
    spans.push(Span::raw(gap));
    spans.push(Span::styled(pad_right(&status, columns.status), status_style));
    Line::from(spans)
}

pub fn render_history(frame: &mut Frame, area: Rect, view: &HistoryView, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let state = FilterLineState {
        text: view.filter(),
        editing: view.editing_filter(),
        matched: view.matched(),
        total: view.total(),
    };
    render_filter_line(frame, Rect::new(area.x, area.y, area.width, 1), state, theme);
    if area.height < 3 {
        return;
    }

    let columns = columns_for(view, theme, usize::from(area.width));
    if view.matched() > 0 {
        frame.render_widget(Paragraph::new(header_line(&columns, theme)), Rect::new(area.x, area.y + 1, area.width, 1));
    }
    let rule = Rect::new(area.x, area.y + 2, area.width, 1);
    hairline(frame, rule, theme);
    accent_underline(frame, rule, 0..area.width, theme);
    if area.height <= HEADER_ROWS {
        return;
    }

    let body = Rect::new(area.x, area.y + HEADER_ROWS, area.width, area.height - HEADER_ROWS);
    if view.total() == 0 || view.matched() == 0 {
        let message = if view.total() == 0 { "no transfers yet" } else { "no matches" };
        frame.render_widget(
            Paragraph::new(Span::styled(message, theme.dim())),
            Rect::new(body.x, body.y, body.width, 1),
        );
        return;
    }

    let cursor = view.cursor();
    let items: Vec<ListItem> = view
        .rows()
        .enumerate()
        .map(|(index, entry)| ListItem::new(row_line(entry, index == cursor, &columns, theme)))
        .collect();
    let list = List::new(items).highlight_style(theme.focus_row());
    let mut state = ListState::default();
    state.select(Some(cursor));
    frame.render_stateful_widget(list, body, &mut state);
}

#[cfg(test)]
mod tests;

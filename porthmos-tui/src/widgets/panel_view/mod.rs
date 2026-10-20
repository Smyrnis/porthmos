use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Local};
pub use porthmos_core::listing::Row;
use porthmos_core::{
    Entry,
    listing::{Listing, SortKey, SortOrder, SortSpec},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        file_list,
        filter_line::{self, FilterLine},
        frame::{accent_underline, hairline},
        history_view::printable,
        text_fit::{truncate_start_with_ellipsis, truncate_with_ellipsis},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Local,
    Remote,
}

impl ActivePanel {
    pub fn toggle(&mut self) {
        *self = match self {
            ActivePanel::Local => ActivePanel::Remote,
            ActivePanel::Remote => ActivePanel::Local,
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Anchor {
    Parent,
    Entry(PathBuf),
}

pub struct PanelView {
    listing: Listing,
    pub cursor: usize,
    pub selected: HashSet<PathBuf>,
    editing_filter: bool,
    anchor: Option<(Anchor, usize)>,
}

impl PanelView {
    pub fn new(path: PathBuf, sort_spec: SortSpec, show_hidden: bool) -> Self {
        Self {
            listing: Listing::new(path, Vec::new(), sort_spec, show_hidden),
            cursor: 0,
            selected: HashSet::new(),
            editing_filter: false,
            anchor: None,
        }
    }

    pub fn from_listing(path: PathBuf, entries: Vec<Entry>) -> Self {
        let mut panel = Self::new(PathBuf::new(), SortSpec::default(), false);
        panel.replace_listing(path, entries);
        panel
    }

    pub fn path(&self) -> &Path {
        self.listing.path()
    }

    pub fn rows(&self) -> &[Row] {
        self.listing.rows()
    }

    pub fn replace_listing(&mut self, path: PathBuf, entries: Vec<Entry>) {
        if path != self.path() {
            self.editing_filter = false;
        }
        self.listing.replace(path, entries);
        self.anchor = None;
        self.selected.clear();
        self.clamp_cursor();
    }

    pub fn toggle_hidden(&mut self) {
        self.listing.toggle_hidden();
        self.clamp_cursor();
    }

    pub fn cycle_sort(&mut self) {
        self.listing.cycle_sort();
        self.clamp_cursor();
    }

    pub fn sort_spec(&self) -> SortSpec {
        self.listing.sort_spec()
    }

    fn apply_filter(&mut self, text: Option<String>) {
        let anchor = match &self.anchor {
            Some((anchor, placed)) if *placed == self.cursor => Some(anchor.clone()),
            _ => self.anchor_at_cursor(),
        };
        self.listing.set_filter(text.as_deref());
        let kept = anchor
            .as_ref()
            .filter(|anchor| self.filter().is_none() || **anchor != Anchor::Parent)
            .and_then(|anchor| self.position_of(anchor));
        let first_match = || self.rows().iter().position(|row| matches!(row, Row::Entry(_)));
        self.cursor = kept.or_else(first_match).unwrap_or(0);
        self.anchor = anchor.map(|anchor| (anchor, self.cursor));
    }

    fn anchor_at_cursor(&self) -> Option<Anchor> {
        match self.rows().get(self.cursor)? {
            Row::Parent => Some(Anchor::Parent),
            Row::Entry(entry) => Some(Anchor::Entry(entry.path.clone())),
        }
    }

    fn position_of(&self, anchor: &Anchor) -> Option<usize> {
        self.rows().iter().position(|row| match (row, anchor) {
            (Row::Parent, Anchor::Parent) => true,
            (Row::Entry(entry), Anchor::Entry(path)) => entry.path == *path,
            _ => false,
        })
    }

    fn filter_status(&self, width: usize) -> Option<String> {
        let (matched, total) = self.listing.match_count();
        filter_line::status(self.editing_filter, self.filter(), matched, total, width)
    }

    fn visible_selected(&self) -> Vec<&Entry> {
        self.rows()
            .iter()
            .filter_map(|row| match row {
                Row::Entry(entry) if self.selected.contains(&entry.path) => Some(entry),
                _ => None,
            })
            .collect()
    }

    pub fn target_path_for_open(&self) -> Option<PathBuf> {
        match self.rows().get(self.cursor) {
            Some(Row::Parent) => self.path().parent().map(Path::to_path_buf),
            Some(Row::Entry(entry)) if entry.is_dir => Some(entry.path.clone()),
            _ => None,
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(Row::Entry(entry)) = self.rows().get(self.cursor) {
            let path = entry.path.clone();
            if !self.selected.remove(&path) {
                self.selected.insert(path);
            }
        }
    }

    pub fn current_entry_name(&self) -> Option<&str> {
        match self.rows().get(self.cursor) {
            Some(Row::Entry(entry)) => Some(entry.name.as_str()),
            _ => None,
        }
    }

    pub fn current_entry(&self) -> Option<&Entry> {
        match self.rows().get(self.cursor) {
            Some(Row::Entry(entry)) => Some(entry),
            _ => None,
        }
    }

    pub fn on_parent_row(&self) -> bool {
        matches!(self.rows().get(self.cursor), Some(Row::Parent))
    }

    pub fn targets(&self) -> Vec<PathBuf> {
        let selected = self.visible_selected();
        if !selected.is_empty() {
            return selected.into_iter().map(|entry| entry.path.clone()).collect();
        }

        match self.rows().get(self.cursor) {
            Some(Row::Entry(entry)) => vec![entry.path.clone()],
            _ => Vec::new(),
        }
    }

    pub fn target_entries(&self) -> Vec<Entry> {
        let selected = self.visible_selected();
        if !selected.is_empty() {
            return selected.into_iter().cloned().collect();
        }

        match self.rows().get(self.cursor) {
            Some(Row::Entry(entry)) => vec![entry.clone()],
            _ => Vec::new(),
        }
    }

    fn clamp_cursor(&mut self) {
        let rows = self.rows().len();
        if rows == 0 {
            self.cursor = 0;
        } else if self.cursor >= rows {
            self.cursor = rows - 1;
        }
    }
}

pub struct PanelContext<'a> {
    pub protocol: &'a str,
    pub location: String,
    pub active: bool,
    pub show_modified: bool,
    pub switch_hint: Option<String>,
    pub now: &'a DateTime<Local>,
}

const HEADER_GAP: usize = 2;
const MIN_LOCATION_WIDTH: usize = 8;
const FOOTER_GAP: usize = 3;

pub fn render_panel(frame: &mut Frame, area: Rect, panel: &PanelView, context: &PanelContext<'_>, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    render_header(frame, area, Some(sort_indicator(panel.sort_spec())), context, theme);
    render_rule(frame, area, context.active, theme);

    let has_footer = area.height >= 4;
    let list_height = area.height.saturating_sub(2 + u16::from(has_footer));
    if list_height > 0 {
        let list_area = Rect::new(area.x, area.y + 2, area.width, list_height);
        let list_context =
            file_list::ListContext { active: context.active, show_modified: context.show_modified, now: context.now };
        file_list::render_file_list(frame, list_area, panel, &list_context, theme);
    }
    if has_footer {
        render_footer(frame, Rect::new(area.x, area.bottom() - 1, area.width, 1), panel, theme);
    }
}

pub fn render_placeholder(frame: &mut Frame, area: Rect, lines: &[String], context: &PanelContext<'_>, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    render_header(frame, area, None, context, theme);
    render_rule(frame, area, context.active, theme);

    let body_height = area.height.saturating_sub(2);
    let shown = lines.len().min(usize::from(body_height));
    let first_row = area.y + 2 + (body_height - shown as u16) / 2;
    for (index, line) in lines.iter().take(shown).enumerate() {
        let text = truncate_with_ellipsis(&printable(line), usize::from(area.width));
        let width = text.width() as u16;
        let x = area.x + (area.width - width) / 2;
        frame.render_widget(
            Paragraph::new(Span::styled(text, theme.dim())),
            Rect::new(x, first_row + index as u16, width, 1),
        );
    }
}

fn render_rule(frame: &mut Frame, area: Rect, active: bool, theme: &Theme) {
    if area.height < 2 {
        return;
    }
    let rule = Rect::new(area.x, area.y + 1, area.width, 1);
    hairline(frame, rule, theme);
    if active {
        accent_underline(frame, rule, 0..area.width, theme);
    }
}

fn render_header(frame: &mut Frame, area: Rect, sort: Option<String>, context: &PanelContext<'_>, theme: &Theme) {
    let width = usize::from(area.width);
    let protocol = printable(context.switch_hint.as_deref().unwrap_or(context.protocol));
    let protocol_width = protocol.width();
    let sort =
        sort.filter(|sort| protocol_width + HEADER_GAP + MIN_LOCATION_WIDTH + HEADER_GAP + sort.width() <= width);
    let sort_reserved = sort.as_ref().map_or(0, |sort| sort.width() + HEADER_GAP);
    let location_budget = width.saturating_sub(protocol_width + HEADER_GAP + sort_reserved);
    let location = truncate_start_with_ellipsis(&printable(&context.location), location_budget);

    let location_style = if context.active { theme.bright() } else { theme.muted() };
    let mut spans = vec![Span::styled(protocol, theme.dim())];
    if !location.is_empty() {
        spans.push(Span::raw(" ".repeat(HEADER_GAP)));
        spans.push(Span::styled(location, location_style));
    }
    let row = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(Paragraph::new(Line::from(spans)), row);
    if let Some(sort) = sort {
        let sort_width = sort.width() as u16;
        let x = area.x + area.width - sort_width;
        frame.render_widget(Paragraph::new(Span::styled(sort, theme.dim())), Rect::new(x, area.y, sort_width, 1));
    }
}

fn footer_left(panel: &PanelView, width: usize) -> String {
    let mut parts = Vec::new();
    if let Some(status) = panel.filter_status(width) {
        parts.push(status);
    }
    let selected = panel.visible_selected();
    if !selected.is_empty() {
        let files: Vec<&Entry> = selected.iter().copied().filter(|entry| !entry.is_dir).collect();
        let mut summary = format!("{} selected", selected.len());
        if !files.is_empty() {
            let total = files.iter().fold(0u64, |sum, entry| sum.saturating_add(entry.size));
            summary.push_str(&format!(" \u{b7} {}", file_list::size_label(total)));
        }
        parts.push(summary);
    }
    printable(&parts.join(&" ".repeat(FOOTER_GAP)))
}

fn items_label(panel: &PanelView) -> String {
    match panel.rows().iter().filter(|row| matches!(row, Row::Entry(_))).count() {
        1 => "1 item".to_string(),
        count => format!("{count} items"),
    }
}

fn render_footer(frame: &mut Frame, area: Rect, panel: &PanelView, theme: &Theme) {
    let width = usize::from(area.width);
    let left = truncate_with_ellipsis(&footer_left(panel, width), width);
    let right = items_label(panel);
    let right_fits = left.width() + FOOTER_GAP + right.width() <= width;
    frame.render_widget(Paragraph::new(Span::styled(left, theme.accent_text())), area);
    if right_fits {
        let right_width = right.width() as u16;
        let x = area.x + area.width - right_width;
        frame.render_widget(Paragraph::new(Span::styled(right, theme.dim())), Rect::new(x, area.y, right_width, 1));
    }
}

fn sort_indicator(spec: SortSpec) -> String {
    let key = match spec.key {
        SortKey::Name => "name",
        SortKey::Size => "size",
    };
    let arrow = match spec.order {
        SortOrder::Ascending => '\u{2191}',
        SortOrder::Descending => '\u{2193}',
    };
    format!("{key} {arrow}")
}

impl FilterLine for PanelView {
    fn filter(&self) -> Option<&str> {
        self.listing.filter()
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
        let mut text = self.filter().unwrap_or_default().to_string();
        text.push(character);
        self.apply_filter(Some(text));
    }

    fn erase_filter(&mut self) {
        let mut text = self.filter().unwrap_or_default().to_string();
        text.pop();
        self.apply_filter(Some(text));
    }

    fn clear_filter(&mut self) {
        self.editing_filter = false;
        self.apply_filter(None);
        self.anchor = None;
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.rows().is_empty() {
            return;
        }

        let max = self.rows().len() as isize - 1;
        let next = (self.cursor as isize + delta).clamp(0, max);
        self.cursor = next as usize;
    }
}

#[cfg(test)]
mod tests;

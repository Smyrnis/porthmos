use std::sync::Arc;

use chrono::{Local, TimeZone};
use porthmos_core::{
    glob_match,
    sync::{SyncAction, SyncFacts, SyncItem, SyncPlan, SyncReason},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use crate::{
    theme::Theme,
    widgets::{
        file_list::{format_size, truncate_name},
        filter_line::{FilterLine, FilterLineState, render_filter_line},
        frame::{accent_underline, hairline},
        history_view::printable,
        text_fit::{pad_left, pad_right},
    },
};

#[derive(Default)]
pub struct SyncView {
    plan: Option<Arc<SyncPlan>>,
    ticked: Vec<bool>,
    actions: Vec<SyncAction>,
    visible: Vec<usize>,
    filter: Option<String>,
    editing_filter: bool,
    cursor: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub ticked: usize,
    pub uploads: usize,
    pub upload_bytes: u64,
    pub downloads: usize,
    pub download_bytes: u64,
    pub skipped: usize,
}

pub fn reason_text(reason: SyncReason) -> &'static str {
    match reason {
        SyncReason::OnlyLocal => "new locally",
        SyncReason::OnlyRemote => "new on remote",
        SyncReason::LocalNewer => "local is newer",
        SyncReason::RemoteNewer => "remote is newer",
        SyncReason::TargetNewer => "target is newer",
        SyncReason::SameTimeDifferentSize => "same time, other size",
        SyncReason::SizeDiffers => "size differs",
        SyncReason::SizeDiffersTimeUnknown => "size differs, no time",
        SyncReason::KindMismatch => "file vs folder",
    }
}

fn matches(item: &SyncItem, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    let path = item.path.to_string_lossy();
    let reason = reason_text(item.reason);
    if filter.contains(['*', '?']) {
        glob_match(filter, &path) || glob_match(filter, reason)
    } else {
        let needle = filter.to_lowercase();
        path.to_lowercase().contains(&needle) || reason.starts_with(&needle) || reason.contains(&format!(" {needle}"))
    }
}

impl SyncView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, plan: Arc<SyncPlan>) {
        self.ticked = plan.items.iter().map(|item| item.ticked).collect();
        self.actions = plan.items.iter().map(|item| item.action).collect();
        self.plan = Some(plan);
        self.filter = None;
        self.editing_filter = false;
        self.cursor = 0;
        self.refilter(None);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn has_plan(&self) -> bool {
        self.plan.is_some()
    }

    pub fn sync_id(&self) -> Option<u64> {
        self.plan.as_ref().map(|plan| plan.sync_id)
    }

    pub fn plan(&self) -> Option<&SyncPlan> {
        self.plan.as_deref()
    }

    pub fn total(&self) -> usize {
        self.ticked.len()
    }

    pub fn matched(&self) -> usize {
        self.visible.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn current(&self) -> Option<usize> {
        self.visible.get(self.cursor).copied()
    }

    pub fn toggle(&mut self) {
        let Some(index) = self.current() else {
            return;
        };
        if self.ticked[index] {
            self.ticked[index] = false;
        } else if self.actions[index] != SyncAction::Skip {
            self.ticked[index] = true;
        } else {
            self.flip_at(index);
        }
    }

    pub fn flip(&mut self) {
        if let Some(index) = self.current() {
            self.flip_at(index);
        }
    }

    fn flip_at(&mut self, index: usize) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        let item = &plan.items[index];
        if !item.flippable {
            return;
        }
        let next = item.action_after(self.actions[index], plan.options.direction);
        self.actions[index] = next;
        self.ticked[index] = next != SyncAction::Skip;
    }

    pub fn tick_all(&mut self) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        for index in self.visible.iter().copied() {
            let default_action = plan.items[index].action;
            if default_action != SyncAction::Skip {
                self.actions[index] = default_action;
                self.ticked[index] = true;
            }
        }
    }

    pub fn untick_all(&mut self) {
        for index in self.visible.clone() {
            self.ticked[index] = false;
        }
    }

    #[cfg(test)]
    pub fn choices(&self) -> Vec<(u32, SyncAction)> {
        let Some(plan) = self.plan.as_ref() else {
            return Vec::new();
        };
        plan.items
            .iter()
            .enumerate()
            .filter(|(index, _)| self.ticked[*index] && self.actions[*index] != SyncAction::Skip)
            .map(|(index, item)| (item.id, self.actions[index]))
            .collect()
    }

    pub fn visible_choices(&self) -> Vec<(u32, SyncAction)> {
        let Some(plan) = self.plan.as_ref() else {
            return Vec::new();
        };
        self.visible
            .iter()
            .copied()
            .filter(|index| self.ticked[*index] && self.actions[*index] != SyncAction::Skip)
            .map(|index| (plan.items[index].id, self.actions[index]))
            .collect()
    }

    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        let Some(plan) = self.plan.as_ref() else {
            return summary;
        };
        for (index, item) in plan.items.iter().enumerate() {
            let action = self.actions[index];
            if !self.ticked[index] || action == SyncAction::Skip {
                summary.skipped += 1;
                continue;
            }
            summary.ticked += 1;
            match action {
                SyncAction::Upload => {
                    summary.uploads += 1;
                    summary.upload_bytes += item.local.map_or(0, |facts| facts.size);
                }
                SyncAction::Download => {
                    summary.downloads += 1;
                    summary.download_bytes += item.remote.map_or(0, |facts| facts.size);
                }
                SyncAction::Skip => {}
            }
        }
        summary
    }

    fn refilter(&mut self, keep: Option<usize>) {
        let filter = self.filter.clone();
        self.visible = match self.plan.as_ref() {
            Some(plan) => plan
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| filter.as_deref().is_none_or(|text| matches(item, text)))
                .map(|(index, _)| index)
                .collect(),
            None => Vec::new(),
        };
        let kept = keep.and_then(|index| self.visible.iter().position(|visible| *visible == index));
        self.cursor = kept.unwrap_or_else(|| self.cursor.min(self.visible.len().saturating_sub(1)));
    }

    fn set_filter(&mut self, text: Option<String>) {
        let keep = self.current();
        self.filter = text.filter(|text| !text.is_empty());
        self.refilter(keep);
    }
}

impl FilterLine for SyncView {
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

fn moment(facts: Option<SyncFacts>) -> String {
    facts
        .and_then(|facts| facts.modified)
        .and_then(|seconds| Local.timestamp_opt(i64::try_from(seconds).ok()?, 0).single())
        .map_or_else(|| "\u{2014}".to_string(), |time| time.format("%Y-%m-%d %H:%M").to_string())
}

const MARKER_WIDTH: usize = 2;
const CHECK_WIDTH: usize = 4;
const ARROW_WIDTH: usize = 2;
const REASON_WIDTH: usize = 22;
const GAP: usize = 2;
const SIZE_WIDTH: usize = 7;
const TIME_WIDTH: usize = 16;
const TIME_CELL_WIDTH: usize = 2 + TIME_WIDTH;
const WITHOUT_PATH_OR_TIMES: usize = MARKER_WIDTH + CHECK_WIDTH + ARROW_WIDTH + REASON_WIDTH + 1 + GAP + SIZE_WIDTH;
const TIME_COLUMNS_WIDTH: usize = 2 * (GAP + TIME_CELL_WIDTH);
const MIN_PATH_WIDTH_WITH_TIMES: usize = 12;
const HEADER_ROWS: u16 = 3;

#[derive(Clone, Copy)]
struct RowLayout {
    path_width: usize,
    with_times: bool,
}

fn row_layout(width: usize) -> RowLayout {
    let with_times = width >= WITHOUT_PATH_OR_TIMES + TIME_COLUMNS_WIDTH + MIN_PATH_WIDTH_WITH_TIMES;
    let taken = WITHOUT_PATH_OR_TIMES + if with_times { TIME_COLUMNS_WIDTH } else { 0 };
    RowLayout { path_width: width.saturating_sub(taken), with_times }
}

fn fitted_path(path: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let shown = truncate_name(&printable(path), width);
    pad_right(&shown, width)
}

fn header_line(layout: RowLayout, theme: &Theme) -> Line<'static> {
    let gap = " ".repeat(GAP);
    let mut text = " ".repeat(MARKER_WIDTH + CHECK_WIDTH + ARROW_WIDTH);
    text.push_str(&format!("{} {}", pad_right("reason", REASON_WIDTH), pad_right("path", layout.path_width)));
    text.push_str(&format!("{gap}{}", pad_left("size", SIZE_WIDTH)));
    if layout.with_times {
        text.push_str(&format!(
            "{gap}{}{gap}{}",
            pad_right("local", TIME_CELL_WIDTH),
            pad_right("remote", TIME_CELL_WIDTH)
        ));
    }
    Line::from(Span::styled(text, theme.dim()))
}

fn row_line(
    item: &SyncItem, ticked: bool, action: SyncAction, focused: bool, layout: RowLayout, theme: &Theme,
) -> Line<'static> {
    let gap = " ".repeat(GAP);
    let marker = if focused { Span::styled("\u{203a} ", theme.accent_text()) } else { Span::raw("  ") };
    let check = if ticked { Span::styled("[x] ", theme.accent_text()) } else { Span::styled("[ ] ", theme.dim()) };
    let arrow = match action {
        SyncAction::Upload => "\u{2191} ",
        SyncAction::Download => "\u{2193} ",
        SyncAction::Skip => "\u{b7} ",
    };
    let size = match action {
        SyncAction::Download => item.remote.or(item.local),
        _ => item.local.or(item.remote),
    }
    .map_or(0, |facts| facts.size);
    let mut text_style = row_style(item, ticked, theme);
    if focused {
        text_style = text_style.add_modifier(Modifier::BOLD);
    }

    let mut spans = vec![
        marker,
        check,
        Span::styled(arrow, theme.muted()),
        Span::styled(pad_right(reason_text(item.reason), REASON_WIDTH), text_style),
        Span::raw(" "),
        Span::styled(fitted_path(&item.path.to_string_lossy(), layout.path_width), text_style),
        Span::raw(gap.clone()),
        Span::styled(pad_left(&format_size(size, false), SIZE_WIDTH), theme.dim()),
    ];
    if layout.with_times {
        let times = format!(
            "{gap}{}{gap}{}",
            pad_right(&format!("L {}", moment(item.local)), TIME_CELL_WIDTH),
            pad_right(&format!("R {}", moment(item.remote)), TIME_CELL_WIDTH),
        );
        spans.push(Span::styled(times, theme.dim()));
    }
    Line::from(spans)
}

fn row_style(item: &SyncItem, ticked: bool, theme: &Theme) -> Style {
    match (item.reason, ticked) {
        (SyncReason::KindMismatch, _) => theme.err_text(),
        (_, true) => theme.text(),
        (SyncReason::TargetNewer | SyncReason::SameTimeDifferentSize | SyncReason::SizeDiffersTimeUnknown, false) => {
            theme.bright()
        }
        (_, false) => theme.dim(),
    }
}

fn summary_text(summary: &Summary) -> String {
    format!(
        "{} ticked \u{b7} \u{2191} {} ({}) \u{b7} \u{2193} {} ({}) \u{b7} {} skipped",
        summary.ticked,
        summary.uploads,
        format_size(summary.upload_bytes, false),
        summary.downloads,
        format_size(summary.download_bytes, false),
        summary.skipped,
    )
}

impl SyncView {
    pub fn top_bar_summary(&self) -> String {
        let summary = summary_text(&self.summary());
        if self.filter.is_some() || self.editing_filter {
            format!("{summary} \u{b7} {} will run", self.visible_choices().len())
        } else {
            summary
        }
    }
}

pub fn render_sync(frame: &mut Frame, area: Rect, view: &SyncView, theme: &Theme) {
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
    let Some(plan) = view.plan.as_ref() else {
        return;
    };
    if area.height < 3 {
        return;
    }

    let layout = row_layout(usize::from(area.width));
    if view.matched() > 0 {
        frame.render_widget(Paragraph::new(header_line(layout, theme)), Rect::new(area.x, area.y + 1, area.width, 1));
    }
    let rule = Rect::new(area.x, area.y + 2, area.width, 1);
    hairline(frame, rule, theme);
    accent_underline(frame, rule, 0..area.width, theme);
    if area.height <= HEADER_ROWS {
        return;
    }

    let body = Rect::new(area.x, area.y + HEADER_ROWS, area.width, area.height - HEADER_ROWS);
    if view.matched() == 0 {
        frame.render_widget(
            Paragraph::new(Span::styled("no matches", theme.dim())),
            Rect::new(body.x, body.y, body.width, 1),
        );
        return;
    }
    let cursor = view.cursor();
    let items: Vec<ListItem> = view
        .visible
        .iter()
        .enumerate()
        .map(|(position, index)| {
            let item = &plan.items[*index];
            ListItem::new(row_line(item, view.ticked[*index], view.actions[*index], position == cursor, layout, theme))
        })
        .collect();
    let list = List::new(items).highlight_style(theme.focus_row());
    let mut state = ListState::default();
    state.select(Some(cursor));
    frame.render_stateful_widget(list, body, &mut state);
}

#[cfg(test)]
mod tests;

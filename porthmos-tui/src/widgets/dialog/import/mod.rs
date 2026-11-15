use std::cell::Cell;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::import::{ImportPlan, Verdict};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        paging::PageRows,
        popup::{footer, render_popup},
        text_fit::truncate_with_ellipsis,
    },
};

const MIN_WIDTH: u16 = 44;
const MAX_WIDTH: u16 = 120;
const SCREEN_MARGIN: u16 = 2;
const CHROME_ROWS: u16 = 5;
const BOTTOM_ROWS: u16 = 3;
const MAX_NAME_COLUMN: usize = 28;
const COLUMN_GAP: &str = "  ";
const TICKED: &str = "[x] ";
const UNTICKED: &str = "[ ] ";
const NO_BOX: &str = "    ";
const NOTHING_TICKED: &str = "Tick at least one connection";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportChoice {
    Pending,
    Run(Vec<u32>),
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    New,
    Exists,
    Unsupported,
    Invalid,
}

struct RowView {
    kind: Kind,
    name: String,
    summary: String,
    group: String,
    status: String,
    detail: String,
}

pub struct ImportDialog {
    title: String,
    rows: Vec<RowView>,
    ticked: Vec<bool>,
    cursor: usize,
    hint: Option<&'static str>,
    scroll: Cell<usize>,
    page: PageRows,
}

impl ImportDialog {
    pub fn new(plan: &ImportPlan) -> Self {
        let rows: Vec<RowView> = plan
            .rows
            .iter()
            .map(|row| {
                let kind = match row.verdict {
                    Verdict::New => Kind::New,
                    Verdict::Exists => Kind::Exists,
                    Verdict::Unsupported(_) => Kind::Unsupported,
                    Verdict::Invalid(_) => Kind::Invalid,
                };
                let status = row.status().unwrap_or_default();
                let detail = [Some(status.clone()), row.notes_line()]
                    .into_iter()
                    .flatten()
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<_>>()
                    .join("; ");
                RowView {
                    kind,
                    name: printable(&row.name),
                    summary: printable(&row.summary()),
                    group: row
                        .candidate
                        .as_ref()
                        .and_then(|found| found.group.as_deref())
                        .map(printable)
                        .unwrap_or_default(),
                    status: printable(&status),
                    detail: printable(&detail),
                }
            })
            .collect();
        let ticked = rows.iter().map(|row| row.kind == Kind::New).collect();
        Self {
            title: format!("Import from {}", plan.source.label()),
            rows,
            ticked,
            cursor: 0,
            hint: None,
            scroll: Cell::new(0),
            page: PageRows::default(),
        }
    }

    pub fn ticked_rows(&self) -> Vec<u32> {
        self.ticked
            .iter()
            .enumerate()
            .filter(|(_, ticked)| **ticked)
            .map(|(index, _)| u32::try_from(index).unwrap_or(u32::MAX))
            .collect()
    }

    fn move_cursor(&mut self, delta: isize) {
        let last = self.rows.len().saturating_sub(1);
        self.cursor = self.cursor.saturating_add_signed(delta).min(last);
    }

    fn set_all_new(&mut self, value: bool) {
        for (ticked, row) in self.ticked.iter_mut().zip(&self.rows) {
            *ticked = value && row.kind == Kind::New;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ImportChoice {
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return ImportChoice::Pending;
        }
        self.hint = None;
        let page = isize::try_from(self.page.get()).unwrap_or(isize::MAX);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return ImportChoice::Cancel,
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::PageUp => self.move_cursor(-page),
            KeyCode::PageDown => self.move_cursor(page),
            KeyCode::Home | KeyCode::Char('g') => self.cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.cursor = self.rows.len().saturating_sub(1),
            KeyCode::Char(' ') => {
                if let Some(ticked) = self.ticked.get_mut(self.cursor)
                    && self.rows[self.cursor].kind == Kind::New
                {
                    *ticked = !*ticked;
                }
            }
            KeyCode::Char('a') => self.set_all_new(true),
            KeyCode::Char('n') => self.set_all_new(false),
            KeyCode::Enter => {
                let chosen = self.ticked_rows();
                if chosen.is_empty() {
                    self.hint = Some(NOTHING_TICKED);
                } else {
                    return ImportChoice::Run(chosen);
                }
            }
            _ => {}
        }
        ImportChoice::Pending
    }

    fn will_be_added(&self) -> usize {
        self.ticked.iter().filter(|ticked| **ticked).count()
    }

    fn not_importable(&self) -> usize {
        self.rows.iter().filter(|row| row.kind != Kind::New).count()
    }
}

fn fit_segments(segments: Vec<(String, Style)>, budget: usize) -> Line<'static> {
    let mut remaining = budget;
    let mut spans = Vec::new();
    for (text, style) in segments {
        if remaining == 0 {
            break;
        }
        let length = text.chars().count();
        if length <= remaining {
            remaining -= length;
            spans.push(Span::styled(text, style));
        } else {
            spans.push(Span::styled(truncate_with_ellipsis(&text, remaining), style));
            break;
        }
    }
    Line::from(spans)
}

fn padded(text: &str, width: usize) -> String {
    let shortened = truncate_with_ellipsis(text, width);
    let missing = width.saturating_sub(shortened.chars().count());
    format!("{shortened}{}", " ".repeat(missing))
}

fn row_line(dialog: &ImportDialog, index: usize, name_column: usize, width: usize, theme: &Theme) -> Line<'static> {
    let row = &dialog.rows[index];
    let tickable = row.kind == Kind::New;
    let (box_text, box_style) = match (tickable, dialog.ticked[index]) {
        (true, true) => (TICKED, theme.accent_text()),
        (true, false) => (UNTICKED, theme.muted()),
        (false, _) => (NO_BOX, theme.dim()),
    };
    let name_style = if tickable { theme.text() } else { theme.dim() };
    let mut segments = vec![
        (box_text.to_string(), box_style),
        (padded(&row.name, name_column), name_style),
        (COLUMN_GAP.to_string(), theme.dim()),
    ];
    match row.kind {
        Kind::New => {
            segments.push((row.summary.clone(), theme.muted()));
            if !row.group.is_empty() {
                segments.push((format!("{COLUMN_GAP}{}", row.group), theme.dim()));
            }
        }
        Kind::Exists => segments.push((row.status.clone(), theme.dim())),
        Kind::Unsupported => segments.push((row.status.clone(), theme.warning_text())),
        Kind::Invalid => segments.push((row.status.clone(), theme.err_text())),
    }
    let line = fit_segments(segments, width);
    if index == dialog.cursor { line.style(theme.focus_row()) } else { line }
}

pub fn render_import(frame: &mut Frame, area: Rect, dialog: &ImportDialog, theme: &Theme) {
    let width = area.width.saturating_sub(2 * SCREEN_MARGIN).clamp(MIN_WIDTH, MAX_WIDTH).min(area.width);
    let wanted = u16::try_from(dialog.rows.len()).unwrap_or(u16::MAX).saturating_add(CHROME_ROWS);
    let height = wanted.min(area.height.saturating_sub(SCREEN_MARGIN).max(1)).min(area.height);
    let inner = render_popup(frame, area, (width, height), Some(&dialog.title), theme);
    if inner.width < 4 || inner.height <= BOTTOM_ROWS {
        return;
    }
    let body_rows = usize::from(inner.height - BOTTOM_ROWS);
    dialog.page.remember(body_rows);
    let mut top = dialog.scroll.get();
    if dialog.cursor < top {
        top = dialog.cursor;
    } else if dialog.cursor >= top + body_rows {
        top = dialog.cursor + 1 - body_rows;
    }
    dialog.scroll.set(top);

    let name_column = dialog.rows.iter().map(|row| row.name.chars().count()).max().unwrap_or(0).min(MAX_NAME_COLUMN);
    let visible: Vec<Line> = (top..dialog.rows.len().min(top + body_rows))
        .map(|index| row_line(dialog, index, name_column, usize::from(inner.width), theme))
        .collect();
    frame.render_widget(Paragraph::new(visible), Rect::new(inner.x, inner.y, inner.width, inner.height - BOTTOM_ROWS));

    let summary_y = inner.bottom() - BOTTOM_ROWS;
    let summary = match dialog.hint {
        Some(hint) => Line::styled(hint.to_string(), theme.warning_text()),
        None => {
            let skipped = dialog.not_importable();
            let extra = if skipped == 0 { String::new() } else { format!(", {skipped} can't be imported") };
            Line::styled(format!("{} will be added{extra}", dialog.will_be_added()), theme.muted())
        }
    };
    frame.render_widget(Paragraph::new(summary), Rect::new(inner.x, summary_y, inner.width, 1));

    let detail = dialog.rows.get(dialog.cursor).map(|row| row.detail.as_str()).unwrap_or("");
    let detail = truncate_with_ellipsis(detail, usize::from(inner.width));
    frame.render_widget(
        Paragraph::new(Line::styled(detail, theme.dim())),
        Rect::new(inner.x, summary_y + 1, inner.width, 1),
    );

    let import_label = format!("import {}", dialog.will_be_added());
    let hints =
        [("space", "tick"), ("a", "all new"), ("n", "none"), ("enter", import_label.as_str()), ("esc", "cancel")];
    footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), &hints, theme);
}

#[cfg(test)]
mod tests;

use porthmos_core::i18n;
use std::cell::Cell;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::{
    Message, VfsPath,
    properties::{
        Digest, Measured,
        digest::matches_expected,
        text::{contents_line, progress_line, size_line, skipped_line},
    },
    size_text,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::types_text;
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
const MAX_WIDTH: u16 = 100;
const SCREEN_MARGIN: u16 = 2;
const LABEL_WIDTH: usize = 10;
const DIGEST_INDENT: usize = 2;
const CURSOR: &str = "\u{258f}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertiesChoice {
    Pending,
    Compute(Vec<VfsPath>),
    CancelRunning,
    Close,
}

enum MeasureState {
    Running(Measured),
    Done(Measured),
    Failed(Message),
    Cancelled,
}

enum HashState {
    Idle,
    Running { done: u64, total: u64 },
    Done(Digest),
    Failed(Message),
    Cancelled,
}

struct FileRow {
    path: VfsPath,
    name: String,
    listed_total: Option<u64>,
    state: HashState,
}

pub struct PropertiesDialog {
    title: String,
    measure: MeasureState,
    files: Vec<FileRow>,
    expected: String,
    editing: bool,
    scroll: usize,
    page: PageRows,
    max_scroll: Cell<usize>,
}

impl PropertiesDialog {
    pub fn new(title: String, files: Vec<(VfsPath, String)>) -> Self {
        let files = files
            .into_iter()
            .map(|(path, name)| FileRow { path, name, listed_total: None, state: HashState::Idle })
            .collect();
        Self {
            title,
            measure: MeasureState::Running(Measured::default()),
            files,
            expected: String::new(),
            editing: false,
            scroll: 0,
            page: PageRows::default(),
            max_scroll: Cell::new(0),
        }
    }

    pub fn set_progress(&mut self, measured: Measured) {
        if matches!(self.measure, MeasureState::Running(_)) {
            self.measure = MeasureState::Running(measured);
        }
    }

    pub fn set_measured(&mut self, result: Result<Measured, Message>) {
        if matches!(self.measure, MeasureState::Cancelled) {
            return;
        }
        self.measure = match result {
            Ok(measured) => MeasureState::Done(measured),
            Err(message) => MeasureState::Failed(message),
        };
    }

    pub fn set_checksum_progress(&mut self, path: &VfsPath, done: u64, total: u64) {
        if let Some(row) = self.files.iter_mut().find(|row| &row.path == path)
            && matches!(row.state, HashState::Running { .. })
        {
            row.listed_total = Some(total);
            row.state = HashState::Running { done, total };
        }
    }

    pub fn set_checksum(&mut self, path: &VfsPath, result: Result<Digest, Message>) {
        if let Some(row) = self.files.iter_mut().find(|row| &row.path == path)
            && matches!(row.state, HashState::Running { .. })
        {
            row.state = match result {
                Ok(digest) => HashState::Done(digest),
                Err(message) => HashState::Failed(message),
            };
        }
    }

    fn hashing(&self) -> bool {
        self.files.iter().any(|row| matches!(row.state, HashState::Running { .. }))
    }

    fn measuring(&self) -> bool {
        matches!(self.measure, MeasureState::Running(_))
    }

    fn has_expected_field(&self) -> bool {
        self.files.len() == 1
    }

    fn cancel_running(&mut self) {
        if self.measuring() {
            self.measure = MeasureState::Cancelled;
        }
        for row in &mut self.files {
            if matches!(row.state, HashState::Running { .. }) {
                row.state = HashState::Cancelled;
            }
        }
    }

    fn edit_expected(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => self.editing = false,
            KeyCode::Backspace => {
                self.expected.pop();
            }
            KeyCode::Char(character) if types_text(key.modifiers) => self.expected.push(character),
            _ => {}
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> PropertiesChoice {
        if self.editing {
            self.edit_expected(key);
            return PropertiesChoice::Pending;
        }
        if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            return PropertiesChoice::Pending;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') if self.measuring() || self.hashing() => {
                self.cancel_running();
                PropertiesChoice::CancelRunning
            }
            KeyCode::Esc | KeyCode::Char('q') => PropertiesChoice::Close,
            KeyCode::Char('s') if !self.files.is_empty() && !self.hashing() => {
                let mut paths = Vec::with_capacity(self.files.len());
                for row in &mut self.files {
                    row.state = HashState::Running { done: 0, total: row.listed_total.unwrap_or(0) };
                    paths.push(row.path.clone());
                }
                PropertiesChoice::Compute(paths)
            }
            KeyCode::Char('e') if self.has_expected_field() => {
                self.editing = true;
                PropertiesChoice::Pending
            }
            _ => {
                self.scroll_by_key(key.code);
                PropertiesChoice::Pending
            }
        }
    }

    fn scroll_by_key(&mut self, code: KeyCode) {
        let page = self.page.get();
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll += 1,
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll += page,
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::Char('g') | KeyCode::Home => self.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.scroll = self.max_scroll.get(),
            _ => {}
        }
        self.scroll = self.scroll.min(self.max_scroll.get());
    }

    fn verdict(&self) -> Option<bool> {
        let [row] = self.files.as_slice() else {
            return None;
        };
        let HashState::Done(digest) = &row.state else {
            return None;
        };
        if self.expected.trim().is_empty() {
            return None;
        }
        Some(matches_expected(&self.expected, &digest.hex))
    }
}

fn digest_rows(hex: &str, width: usize) -> Vec<String> {
    if width == 0 || width >= hex.chars().count() {
        return vec![hex.to_string()];
    }
    let characters: Vec<char> = hex.chars().collect();
    characters.chunks(width).map(|chunk| chunk.iter().collect()).collect()
}

fn padded_label(label: &str) -> String {
    let padded = format!("{label:<LABEL_WIDTH$}");
    if label.chars().count() >= LABEL_WIDTH { format!("{padded} ") } else { padded }
}

fn labelled(label: &str, value: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(padded_label(label), theme.muted()), Span::styled(value.to_string(), theme.text())])
}

fn measure_lines(dialog: &PropertiesDialog, theme: &Theme) -> Vec<Line<'static>> {
    match &dialog.measure {
        MeasureState::Running(measured) => vec![
            labelled(&i18n::text("props.size"), &size_line(measured.bytes), theme),
            Line::styled(progress_line(measured), theme.dim()),
        ],
        MeasureState::Done(measured) => {
            let mut lines = vec![
                labelled(&i18n::text("props.size"), &size_line(measured.bytes), theme),
                labelled(&i18n::text("props.contents"), &contents_line(measured), theme),
            ];
            if let Some(skipped) = skipped_line(measured) {
                lines.push(labelled(&i18n::text("props.skipped"), &skipped, theme));
            }
            lines
        }
        MeasureState::Failed(message) => vec![Line::styled(printable(&message.one_line()), theme.err_text())],
        MeasureState::Cancelled => vec![labelled(&i18n::text("props.size"), &i18n::text("props.cancelled"), theme)],
    }
}

fn file_lines(row: &FileRow, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let name = printable(&row.name);
    let (status, style) = match &row.state {
        HashState::Idle => (i18n::text("props.not_computed"), theme.dim()),
        HashState::Running { done, total } if *total == 0 => {
            (i18n::text_with("props.reading", &[("done", &size_text(*done))]), theme.dim())
        }
        HashState::Running { done, total } => (
            i18n::text_with("props.progress", &[("done", &size_text(*done)), ("total", &size_text(*total))]),
            theme.dim(),
        ),
        HashState::Failed(message) => (printable(&message.one_line()), theme.err_text()),
        HashState::Cancelled => (i18n::text("props.cancelled"), theme.dim()),
        HashState::Done(digest) => match row.listed_total {
            Some(total) if total != digest.bytes => {
                (i18n::text_with("props.bytes_read", &[("count", &digest.bytes.to_string())]), theme.dim())
            }
            _ => (String::new(), theme.dim()),
        },
    };
    let mut head = vec![Span::styled(name, theme.text())];
    if !status.is_empty() {
        head.push(Span::styled(format!("  {status}"), style));
    }
    let mut lines = vec![Line::from(head)];
    if let HashState::Done(digest) = &row.state {
        let available = width.saturating_sub(DIGEST_INDENT);
        for part in digest_rows(&digest.hex, available) {
            lines.push(Line::styled(format!("{}{part}", " ".repeat(DIGEST_INDENT)), theme.accent_text()));
        }
    }
    lines
}

fn expected_lines(dialog: &PropertiesDialog, theme: &Theme) -> Vec<Line<'static>> {
    if !dialog.has_expected_field() {
        return Vec::new();
    }
    let cursor = if dialog.editing { CURSOR } else { "" };
    let mut lines = vec![Line::from(vec![
        Span::styled(padded_label(&i18n::text("props.expected")), theme.muted()),
        Span::styled(format!("{}{cursor}", printable(&dialog.expected)), theme.text()),
    ])];
    match dialog.verdict() {
        Some(true) => lines.push(Line::styled(i18n::text("props.matches"), theme.ok_text())),
        Some(false) => lines.push(Line::styled(i18n::text("props.no_match"), theme.err_text())),
        None => {}
    }
    lines
}

fn hints(dialog: &PropertiesDialog) -> Vec<(&'static str, String)> {
    if dialog.editing {
        return vec![("enter", i18n::text("footer.done")), ("esc", i18n::text("footer.done"))];
    }
    if dialog.measuring() || dialog.hashing() {
        return vec![("esc", i18n::text("footer.cancel"))];
    }
    let mut pairs = Vec::new();
    if !dialog.files.is_empty() {
        pairs.push(("s", i18n::text("footer.sha256")));
    }
    if dialog.has_expected_field() {
        pairs.push(("e", i18n::text("footer.expected")));
    }
    if dialog.max_scroll.get() > 0 {
        pairs.push(("j/k", i18n::text("footer.scroll")));
    }
    pairs.push(("esc", i18n::text("footer.close")));
    pairs
}

pub fn render_properties(frame: &mut Frame, area: Rect, dialog: &PropertiesDialog, theme: &Theme) {
    let width = area.width.saturating_sub(2 * SCREEN_MARGIN).clamp(MIN_WIDTH, MAX_WIDTH).min(area.width);
    let content_width = usize::from(width.saturating_sub(4));
    let mut lines = measure_lines(dialog, theme);
    if !dialog.files.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled("SHA-256", theme.muted()));
        for row in &dialog.files {
            lines.extend(file_lines(row, content_width, theme));
        }
        lines.extend(expected_lines(dialog, theme));
    }
    let wanted = u16::try_from(lines.len() + 3).unwrap_or(u16::MAX);
    let height = wanted.min(area.height.saturating_sub(SCREEN_MARGIN).max(1)).min(area.height);
    let inner = render_popup(frame, area, (width, height), Some(&dialog.title), theme);
    if inner.width == 0 || inner.height < 2 {
        return;
    }
    let body_rows = inner.height.saturating_sub(1);
    dialog.page.remember(usize::from(body_rows));
    dialog.max_scroll.set(lines.len().saturating_sub(usize::from(body_rows)));
    let clip = usize::from(inner.width);
    let shown: Vec<Line> = lines
        .into_iter()
        .skip(dialog.scroll.min(dialog.max_scroll.get()))
        .take(usize::from(body_rows))
        .map(|line| {
            let text: String = line.spans.iter().map(|span| span.content.as_ref()).collect();
            if text.chars().count() <= clip {
                line
            } else {
                Line::styled(
                    truncate_with_ellipsis(&text, clip),
                    line.spans.first().map_or_else(|| theme.text(), |span| span.style),
                )
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(shown), Rect::new(inner.x, inner.y, inner.width, body_rows));
    footer(frame, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1), &hints(dialog), theme);
}

#[cfg(test)]
mod tests;

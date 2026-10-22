use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use porthmos_core::preview::{Preview, PreviewContent};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    theme::Theme,
    widgets::{
        file_list::format_size,
        frame::{fill_background, hairline},
        history_view::printable,
        text_fit::{ELLIPSIS, pad_left},
    },
};

const GAP: usize = 2;
const HINT_GAP: usize = 3;
const HEX_BYTES: usize = 16;
const TAB: usize = 4;
const ZERO_WIDTH_JOINER: char = '\u{200d}';

pub enum PreviewState {
    Loading,
    Failed(String),
    Ready(Arc<Preview>),
}

struct Row {
    number: Option<usize>,
    text: String,
    dim: bool,
}

struct Layout {
    wrap: bool,
    width: usize,
    rows: Vec<Row>,
    gutter: usize,
}

enum Anchor {
    Line(usize),
    Bottom,
}

pub struct PreviewView {
    pub name: String,
    pub state: PreviewState,
    lines: Vec<String>,
    top: Cell<usize>,
    wrap: bool,
    layout: RefCell<Option<Layout>>,
    max_top: Cell<usize>,
    body_rows: Cell<usize>,
    #[cfg(test)]
    layouts: Cell<usize>,
}

impl PreviewView {
    pub fn loading(name: String) -> Self {
        Self {
            name,
            state: PreviewState::Loading,
            lines: Vec::new(),
            top: Cell::new(0),
            wrap: false,
            layout: RefCell::new(None),
            max_top: Cell::new(0),
            body_rows: Cell::new(0),
            #[cfg(test)]
            layouts: Cell::new(0),
        }
    }

    pub fn finish(&mut self, result: Result<Arc<Preview>, String>) {
        self.lines = match &result {
            Ok(preview) => match &preview.content {
                PreviewContent::Text(text) => text_lines(text),
                PreviewContent::Binary(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        self.state = match result {
            Ok(preview) => PreviewState::Ready(preview),
            Err(message) => PreviewState::Failed(message),
        };
        self.top.set(0);
        *self.layout.borrow_mut() = None;
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, PreviewState::Loading)
    }

    #[cfg(test)]
    pub fn top(&self) -> usize {
        self.top.get()
    }

    #[cfg(test)]
    pub fn wrap(&self) -> bool {
        self.wrap
    }

    #[cfg(test)]
    pub fn layouts(&self) -> usize {
        self.layouts.get()
    }

    pub fn scroll(&mut self, delta: isize) {
        let max = self.max_top.get();
        self.top.set((self.top.get().min(max) as isize + delta).clamp(0, max as isize) as usize);
    }

    pub fn page(&mut self, pages: isize) {
        let step = self.body_rows.get().saturating_sub(1).max(1) as isize;
        self.scroll(pages * step);
    }

    pub fn go_top(&mut self) {
        self.top.set(0);
    }

    pub fn go_bottom(&mut self) {
        self.top.set(self.max_top.get());
    }

    pub fn toggle_wrap(&mut self) {
        self.wrap = !self.wrap;
    }

    fn is_binary(&self) -> bool {
        matches!(&self.state, PreviewState::Ready(preview) if matches!(preview.content, PreviewContent::Binary(_)))
    }

    fn anchor_in(&self, layout: &Layout) -> Anchor {
        let top = self.top.get();
        if self.max_top.get() > 0 && top >= self.max_top.get() {
            return Anchor::Bottom;
        }
        let end = (top + 1).min(layout.rows.len());
        let line = layout.rows[..end].iter().rev().find_map(|row| row.number).unwrap_or(1);
        Anchor::Line(line)
    }

    fn ensure_layout(&self, width: usize) {
        let mut slot = self.layout.borrow_mut();
        if slot.as_ref().is_some_and(|layout| layout.wrap == self.wrap && layout.width == width) {
            return;
        }
        let anchor = slot.as_ref().map(|old| self.anchor_in(old));
        let layout = text_layout(&self.lines, self.wrap, width);
        match anchor {
            Some(Anchor::Line(line)) => {
                if let Some(index) = layout.rows.iter().position(|row| row.number == Some(line)) {
                    self.top.set(index);
                }
            }
            Some(Anchor::Bottom) => self.top.set(usize::MAX),
            None => {}
        }
        *slot = Some(layout);
        #[cfg(test)]
        self.layouts.set(self.layouts.get() + 1);
    }
}

fn expand_tabs(line: &str) -> String {
    let mut expanded = String::with_capacity(line.len());
    let mut column = 0;
    for character in line.chars() {
        if character == '\t' {
            let spaces = TAB - column % TAB;
            expanded.push_str(&" ".repeat(spaces));
            column += spaces;
        } else {
            expanded.push(character);
            column += character.width().unwrap_or(0);
        }
    }
    expanded
}

fn text_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let body = text.strip_suffix('\n').unwrap_or(text);
    body.split('\n').map(|line| printable(&expand_tabs(line.strip_suffix('\r').unwrap_or(line)))).collect()
}

fn clusters(line: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut start = 0;
    let mut previous: Option<char> = None;
    for (index, character) in line.char_indices() {
        let joins = character.width() == Some(0) || previous == Some(ZERO_WIDTH_JOINER);
        if index > 0 && !joins {
            found.push(&line[start..index]);
            start = index;
        }
        previous = Some(character);
    }
    if !line.is_empty() {
        found.push(&line[start..]);
    }
    found
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut used = 0;
    for cluster in clusters(line) {
        let cell = cluster.width();
        if used + cell > width && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push_str(cluster);
        used += cell;
    }
    chunks.push(current);
    chunks
}

fn fit_line(line: &str, budget: usize) -> String {
    let parts = clusters(line);
    if parts.iter().map(|cluster| cluster.width()).sum::<usize>() <= budget {
        return line.to_string();
    }
    if budget == 0 {
        return String::new();
    }
    let mut kept = String::new();
    let mut used = 0;
    for cluster in parts {
        if used + cluster.width() > budget - 1 {
            break;
        }
        kept.push_str(cluster);
        used += cluster.width();
    }
    kept.push(ELLIPSIS);
    kept
}

pub fn hex_row(offset: usize, bytes: &[u8]) -> String {
    let mut row = format!("{offset:08x}  ");
    for index in 0..HEX_BYTES {
        if index == HEX_BYTES / 2 {
            row.push(' ');
        }
        match bytes.get(index) {
            Some(byte) => row.push_str(&format!("{byte:02x} ")),
            None => row.push_str("   "),
        }
    }
    row.push_str(" |");
    row.extend(
        bytes.iter().take(HEX_BYTES).map(|byte| if (0x20..0x7f).contains(byte) { char::from(*byte) } else { '.' }),
    );
    row.push('|');
    row
}

fn size_text(size: u64) -> String {
    if size < 1024 { format!("{size} B") } else { format_size(size, false) }
}

fn info_text(view: &PreviewView) -> String {
    let PreviewState::Ready(preview) = &view.state else {
        return String::new();
    };
    if preview.truncated {
        return match preview.size {
            Some(size) => format!("first {} of {}", size_text(preview.shown as u64), size_text(size)),
            None => format!("first {}", size_text(preview.shown as u64)),
        };
    }
    let kind = if view.is_binary() { "binary" } else { "text" };
    format!("{kind} \u{b7} {}", size_text(preview.size.unwrap_or(preview.shown as u64)))
}

fn digits(number: usize) -> usize {
    number.max(1).ilog10() as usize + 1
}

fn text_layout(lines: &[String], wrap: bool, width: usize) -> Layout {
    if lines.is_empty() {
        let rows = vec![Row { number: None, text: "(empty file)".to_string(), dim: true }];
        return Layout { wrap, width, rows, gutter: 0 };
    }
    let gutter = digits(lines.len());
    let content = width.saturating_sub(gutter + GAP).max(1);
    let mut rows = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if wrap {
            for (part, chunk) in wrap_line(line, content).into_iter().enumerate() {
                rows.push(Row { number: (part == 0).then_some(index + 1), text: chunk, dim: false });
            }
        } else {
            rows.push(Row { number: Some(index + 1), text: fit_line(line, content), dim: false });
        }
    }
    Layout { wrap, width, rows, gutter }
}

fn hex_lines(bytes: &[u8], from: usize, count: usize, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    (from..bytes.len().div_ceil(HEX_BYTES).min(from + count))
        .map(|row| {
            let start = row * HEX_BYTES;
            let end = (start + HEX_BYTES).min(bytes.len());
            Line::styled(fit_line(&hex_row(start, &bytes[start..end]), width), theme.text())
        })
        .collect()
}

fn text_window(layout: &Layout, from: usize, count: usize, theme: &Theme) -> Vec<Line<'static>> {
    layout
        .rows
        .iter()
        .skip(from)
        .take(count)
        .map(|row| {
            let style = if row.dim { theme.dim() } else { theme.text() };
            let gutter = match row.number {
                Some(number) => pad_left(&number.to_string(), layout.gutter),
                None => " ".repeat(layout.gutter),
            };
            Line::from(vec![
                Span::styled(gutter, theme.dim()),
                Span::raw(" ".repeat(if layout.gutter > 0 { GAP } else { 0 })),
                Span::styled(row.text.clone(), style),
            ])
        })
        .collect()
}

fn footer_line(view: &PreviewView, width: usize, percent: Option<usize>, theme: &Theme) -> Line<'static> {
    let mut items: Vec<(&str, &str)> = Vec::new();
    if matches!(view.state, PreviewState::Ready(_)) {
        items.push(("\u{2191}\u{2193}", "scroll"));
        items.push(("space", "page"));
        items.push(("g/G", "top/bottom"));
        if !view.is_binary() {
            items.push(("w", "wrap"));
        }
    }
    items.push(("esc", "close"));
    let right = percent.map(|value| format!("{value}%")).unwrap_or_default();
    let budget = width.saturating_sub(right.width() + 1);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for (index, (key, label)) in items.iter().enumerate() {
        let gap = if index == 0 { 0 } else { HINT_GAP };
        let needed = gap + key.width() + 1 + label.width();
        if used + needed > budget {
            break;
        }
        if gap > 0 {
            spans.push(Span::raw(" ".repeat(gap)));
        }
        spans.push(Span::styled((*key).to_string(), theme.bright()));
        spans.push(Span::styled(format!(" {label}"), theme.dim()));
        used += needed;
    }
    if !right.is_empty() {
        spans.push(Span::raw(" ".repeat(width.saturating_sub(used + right.width()))));
        spans.push(Span::styled(right, theme.dim()));
    }
    Line::from(spans)
}

fn title_line(view: &PreviewView, width: usize, theme: &Theme) -> Line<'static> {
    let info = info_text(view);
    let info_width = if info.is_empty() { 0 } else { info.width() + 2 };
    let name_budget = width.saturating_sub(info_width + "preview \u{b7} ".width());
    let name = fit_line(&printable(&view.name), name_budget);
    let left_width = "preview \u{b7} ".width() + name.width();
    let mut spans = vec![Span::styled("preview \u{b7} ", theme.dim()), Span::styled(name, theme.bright())];
    if !info.is_empty() && left_width + info_width <= width {
        spans.push(Span::raw(" ".repeat(width - left_width - info.width())));
        spans.push(Span::styled(info, theme.dim()));
    }
    Line::from(spans)
}

pub fn render_preview(frame: &mut Frame, area: Rect, view: &PreviewView, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(Clear, area);
    fill_background(frame, area, theme);
    let width = usize::from(area.width);
    frame.render_widget(Paragraph::new(title_line(view, width, theme)), Rect::new(area.x, area.y, area.width, 1));
    if area.height < 2 {
        return;
    }
    hairline(frame, Rect::new(area.x, area.y + 1, area.width, 1), theme);
    if area.height < 4 {
        view.body_rows.set(0);
        view.max_top.set(0);
        return;
    }
    let body = Rect::new(area.x, area.y + 2, area.width, area.height - 3);
    let visible = usize::from(body.height);
    view.body_rows.set(visible);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut percent = None;
    match &view.state {
        PreviewState::Loading => {
            view.max_top.set(0);
            lines.push(Line::styled("loading\u{2026}", theme.dim()));
        }
        PreviewState::Failed(message) => {
            view.max_top.set(0);
            for line in message.lines() {
                lines.push(Line::styled(fit_line(&printable(line), width), theme.err_text()));
            }
        }
        PreviewState::Ready(preview) => {
            let layout = match &preview.content {
                PreviewContent::Text(_) => {
                    view.ensure_layout(width);
                    view.layout.borrow()
                }
                PreviewContent::Binary(_) => view.layout.borrow(),
            };
            let total = match (&preview.content, layout.as_ref()) {
                (PreviewContent::Binary(bytes), _) => bytes.len().div_ceil(HEX_BYTES),
                (_, Some(layout)) => layout.rows.len(),
                _ => 0,
            };
            let max_top = total.saturating_sub(visible);
            view.max_top.set(max_top);
            let top = view.top.get().min(max_top);
            view.top.set(top);
            percent = Some(((top + visible) * 100 / total.max(1)).min(100));
            lines = match (&preview.content, layout.as_ref()) {
                (PreviewContent::Binary(bytes), _) => hex_lines(bytes, top, visible, width, theme),
                (_, Some(layout)) => text_window(layout, top, visible, theme),
                _ => Vec::new(),
            };
        }
    }
    lines.truncate(visible);
    frame.render_widget(Paragraph::new(lines), body);
    frame.render_widget(
        Paragraph::new(footer_line(view, width, percent, theme)),
        Rect::new(area.x, area.y + area.height - 1, area.width, 1),
    );
}

#[cfg(test)]
mod tests;

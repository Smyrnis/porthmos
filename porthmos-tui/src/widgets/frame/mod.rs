use std::ops::Range;

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;

const PAIR_GAP: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub key: String,
    pub label: &'static str,
}

impl Hint {
    fn width(&self) -> usize {
        self.key.width() + 1 + self.label.width()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hints {
    pub entries: Vec<Hint>,
    pub pinned: Option<Hint>,
    pub right: Option<Hint>,
}

struct VisibleHints<'a> {
    entries: &'a [Hint],
    pinned: Option<&'a Hint>,
    right: Option<&'a Hint>,
}

fn visible_hints(hints: &Hints, width: usize) -> VisibleHints<'_> {
    let pinned = hints.pinned.as_ref();
    let pinned_width = pinned.map_or(0, Hint::width);
    let gap_before_right = if pinned.is_some() { PAIR_GAP } else { 0 };
    let right = hints.right.as_ref().filter(|right| pinned_width + gap_before_right + right.width() <= width);
    let reserved = pinned_width + right.map_or(0, |right| right.width() + gap_before_right);

    let mut used = reserved;
    let mut kept = 0;
    for entry in &hints.entries {
        let needed = entry.width() + PAIR_GAP;
        if used + needed > width {
            break;
        }
        used += needed;
        kept += 1;
    }
    VisibleHints { entries: &hints.entries[..kept], pinned, right }
}

pub fn fill_background(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(Block::new().style(theme.text().bg(theme.bg)), area);
}

pub fn clear_popup(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(Clear, area);
    fill_background(frame, area, theme);
}

pub fn hairline(frame: &mut Frame, area: Rect, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let row = Rect::new(area.x, area.y, area.width, 1);
    let rule = "\u{2500}".repeat(usize::from(area.width));
    frame.render_widget(Paragraph::new(Span::styled(rule, theme.hairline())), row);
}

pub fn accent_underline(frame: &mut Frame, rule: Rect, columns: Range<u16>, theme: &Theme) {
    if rule.width == 0 || rule.height == 0 {
        return;
    }
    let start = columns.start.min(rule.width);
    let end = columns.end.min(rule.width);
    let accent = theme.accent_text();
    for column in start..end {
        frame.buffer_mut()[(rule.x + column, rule.y)].set_style(accent);
    }
}

pub fn top_bar(frame: &mut Frame, area: Rect, left: Line<'_>, right: Line<'_>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let left_width = left.width();
    let right_width = right.width();
    let width = usize::from(area.width);
    frame.render_widget(Paragraph::new(left), area);
    if right_width > 0 && left_width + PAIR_GAP + right_width <= width {
        let x = area.x + (width - right_width) as u16;
        frame.render_widget(Paragraph::new(right), Rect::new(x, area.y, right_width as u16, 1));
    }
}

fn pair_spans<'a>(hint: &'a Hint, theme: &Theme) -> [Span<'a>; 3] {
    [Span::styled(hint.key.as_str(), theme.bright()), Span::raw(" "), Span::styled(hint.label, theme.dim())]
}

pub fn hint_bar(frame: &mut Frame, area: Rect, hints: &Hints, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let visible = visible_hints(hints, usize::from(area.width));
    let mut spans: Vec<Span> = Vec::new();
    for entry in visible.entries {
        spans.extend(pair_spans(entry, theme));
        spans.push(Span::raw(" ".repeat(PAIR_GAP)));
    }
    if let Some(pinned) = visible.pinned {
        spans.extend(pair_spans(pinned, theme));
    }
    let row = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(Paragraph::new(Line::from(spans)), row);

    if let Some(right) = visible.right {
        let width = right.width() as u16;
        let x = area.x + area.width - width;
        frame.render_widget(
            Paragraph::new(Line::from(pair_spans(right, theme).to_vec())),
            Rect::new(x, area.y, width, 1),
        );
    }
}

#[cfg(test)]
mod tests;

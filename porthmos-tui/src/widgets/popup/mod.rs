use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{frame::clear_popup, history_view::printable, text_fit::take_width},
};

const PAIR_GAP: usize = 3;
const SIDE_PADDING: u16 = 1;
const MIN_CONTENT_WIDTH: usize = 20;
const MAX_CONTENT_WIDTH: usize = 76;
const BORDER_AND_PADDING: usize = 4;

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
}

pub fn render_popup(frame: &mut Frame, screen: Rect, size: (u16, u16), title: Option<&str>, theme: &Theme) -> Rect {
    frame.buffer_mut().set_style(screen, theme.backdrop());
    let popup = centered(screen, size.0, size.1);
    if popup.width == 0 || popup.height == 0 {
        return Rect::new(popup.x, popup.y, 0, 0);
    }
    clear_popup(frame, popup, theme);
    let mut block = Block::bordered().border_style(theme.popup_border());
    if let Some(title) = title {
        block = block.title(Span::styled(format!(" {} ", printable(title)), theme.muted()));
    }
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if inner.width < 2 * SIDE_PADDING {
        return Rect::new(inner.x, inner.y, 0, inner.height);
    }
    Rect::new(inner.x + SIDE_PADDING, inner.y, inner.width - 2 * SIDE_PADDING, inner.height)
}

pub fn footer_text(pairs: &[(&str, &str)]) -> String {
    let parts: Vec<String> = pairs.iter().map(|(key, label)| format!("{key} {label}")).collect();
    parts.join(&" ".repeat(PAIR_GAP))
}

pub fn footer(frame: &mut Frame, area: Rect, pairs: &[(&str, &str)], theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut spans: Vec<Span> = Vec::new();
    let mut used = 0;
    for (key, label) in pairs {
        let width = key.width() + 1 + label.width();
        let gap = if spans.is_empty() { 0 } else { PAIR_GAP };
        if used + gap + width > usize::from(area.width) {
            break;
        }
        if gap > 0 {
            spans.push(Span::raw(" ".repeat(gap)));
        }
        spans.push(Span::styled(key.to_string(), theme.bright()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(label.to_string(), theme.dim()));
        used += gap + width;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(area.x, area.y, area.width, 1));
}

pub fn option_spans(key: &str, label: &str, focused: bool, theme: &Theme) -> Vec<Span<'static>> {
    let marker = if focused { Span::styled("\u{203a} ", theme.accent_text()) } else { Span::raw("  ") };
    let label_style = if focused { theme.bright() } else { theme.dim() };
    vec![
        marker,
        Span::styled(key.to_string(), theme.bright()),
        Span::raw(" "),
        Span::styled(label.to_string(), label_style),
    ]
}

pub fn content_width(lines: &[&str]) -> u16 {
    let longest = lines.iter().map(|line| line.width()).max().unwrap_or(0);
    (longest + BORDER_AND_PADDING).clamp(MIN_CONTENT_WIDTH, MAX_CONTENT_WIDTH) as u16
}

pub fn wrap_line(line: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let mut rest = word;
        while rest.width() > width {
            if !current.is_empty() {
                rows.push(std::mem::take(&mut current));
            }
            let mut head = take_width(rest, width);
            if head.is_empty() {
                head = rest.chars().take(1).collect();
            }
            rest = &rest[head.len()..];
            rows.push(head);
        }
        if rest.is_empty() {
            continue;
        }
        let separator = usize::from(!current.is_empty());
        if current.width() + separator + rest.width() > width {
            rows.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(rest);
    }
    if !current.is_empty() || rows.is_empty() {
        rows.push(current);
    }
    rows
}

#[cfg(test)]
mod tests;

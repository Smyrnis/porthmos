use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    theme::Theme,
    widgets::{
        history_view::printable,
        layout::SIDE_PADDING,
        notifications::Severity,
        text_fit::{ELLIPSIS, take_width},
    },
};

const DOT: &str = "\u{2022}";
const MAX_OUTER_WIDTH: u16 = 60;
const MAX_LINES: usize = 3;
const BORDER_AND_PADDING_AND_DOT: u16 = 6;
const TOP_OFFSET: u16 = 2;

fn break_into_rows(word: &str, width: usize, rows: &mut Vec<String>, current: &mut String) {
    for character in word.chars() {
        let character_width = character.width().unwrap_or(0);
        if character_width > width {
            continue;
        }
        if current.width() + character_width > width {
            rows.push(std::mem::take(current));
        }
        current.push(character);
    }
}

fn wrap(line: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut current = String::new();
    for word in line.split(' ').filter(|word| !word.is_empty()) {
        if current.is_empty() {
            break_into_rows(word, width, &mut rows, &mut current);
        } else if current.width() + 1 + word.width() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            rows.push(std::mem::take(&mut current));
            break_into_rows(word, width, &mut rows, &mut current);
        }
    }
    rows.push(current);
    rows
}

pub fn toast_lines(message: &str, width: usize, max_lines: usize) -> Vec<String> {
    if width == 0 || max_lines == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> =
        message.trim_end().split('\n').flat_map(|line| wrap(&printable(line.trim_end_matches('\r')), width)).collect();
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            let mut kept = take_width(last, width - 1);
            kept.push(ELLIPSIS);
            *last = kept;
        }
    }
    lines
}

pub fn render_toast(frame: &mut Frame, area: Rect, severity: Severity, message: &str, theme: &Theme) {
    let outer_budget = area.width.saturating_sub(2 * SIDE_PADDING).min(MAX_OUTER_WIDTH);
    let text_budget = usize::from(outer_budget.saturating_sub(BORDER_AND_PADDING_AND_DOT));
    let lines = toast_lines(message, text_budget, MAX_LINES);
    if lines.is_empty() {
        return;
    }
    let text_width = lines.iter().map(|line| line.width()).max().unwrap_or(0).max(1) as u16;
    let outer_width = text_width + BORDER_AND_PADDING_AND_DOT;
    let height = lines.len() as u16 + 2;
    if area.y + TOP_OFFSET + height > area.bottom() {
        return;
    }
    let popup = Rect::new(area.right() - SIDE_PADDING - outer_width, area.y + TOP_OFFSET, outer_width, height);

    let dot_style = match severity {
        Severity::Info => theme.muted(),
        Severity::Warning => theme.warning_text(),
        Severity::Error => theme.err_text(),
    };
    let rows: Vec<Line> = lines
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let lead = if index == 0 { Span::styled(DOT, dot_style) } else { Span::raw(" ") };
            Line::from(vec![lead, Span::raw(" "), Span::styled(text, theme.text())])
        })
        .collect();

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.popup_border())
        .padding(Padding::horizontal(1))
        .style(theme.toast_surface());
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(rows).block(block).style(theme.toast_surface()), popup);
}

#[cfg(test)]
mod tests;

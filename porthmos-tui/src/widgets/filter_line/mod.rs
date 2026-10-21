use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{history_view::printable, text_fit::truncate_start_with_ellipsis},
};

const CARET: &str = "\u{2588}";
const COUNT_GAP: usize = 3;

pub trait FilterLine {
    fn filter(&self) -> Option<&str>;
    fn editing_filter(&self) -> bool;
    fn start_filter(&mut self);
    fn finish_filter(&mut self);
    fn type_filter(&mut self, character: char);
    fn erase_filter(&mut self);
    fn clear_filter(&mut self);
    fn move_cursor(&mut self, delta: isize);
}

pub fn status(editing: bool, text: Option<&str>, matched: usize, total: usize, width: usize) -> Option<String> {
    let (prefix, text, suffix) = match (editing, text) {
        (true, text) => ("/", text.unwrap_or_default(), format!("\u{2588} ({matched} of {total})")),
        (false, Some(text)) => ("filter: ", text, format!(" ({matched} of {total})")),
        (false, None) => return None,
    };
    let room = width.saturating_sub(prefix.width() + suffix.width());
    let shown = truncate_start_with_ellipsis(text, room);
    Some(format!("{prefix}{shown}{suffix}"))
}

#[derive(Debug, Clone, Copy)]
pub struct FilterLineState<'a> {
    pub text: Option<&'a str>,
    pub editing: bool,
    pub matched: usize,
    pub total: usize,
}

pub fn render_filter_line(frame: &mut Frame, area: Rect, state: FilterLineState<'_>, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let count =
        if state.text.is_some() { format!("{} of {}", state.matched, state.total) } else { state.total.to_string() };

    let mut spans = vec![Span::styled("/", theme.accent_text()), Span::raw(" ")];
    match (state.text, state.editing) {
        (Some(text), editing) => {
            let budget = width.saturating_sub(2 + count.width() + COUNT_GAP + 1);
            spans.push(Span::styled(truncate_start_with_ellipsis(&printable(text), budget), theme.bright()));
            if editing {
                spans.push(Span::styled(CARET, theme.bright()));
            }
        }
        (None, true) => spans.push(Span::styled(CARET, theme.bright())),
        (None, false) => spans.push(Span::styled("filter", theme.dim())),
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);

    let count_width = count.width();
    if width >= 2 + 6 + COUNT_GAP + count_width {
        let x = area.x + area.width - count_width as u16;
        frame.render_widget(
            Paragraph::new(Span::styled(count, theme.dim())),
            Rect::new(x, area.y, count_width as u16, 1),
        );
    }
}

#[cfg(test)]
mod tests;

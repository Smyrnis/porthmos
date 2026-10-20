use chrono::{DateTime, Local};
use porthmos_core::Entry;
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
        file_list::{dates::format_modified_with_time, permissions_string, size_label},
        history_view::printable,
        text_fit::truncate_with_ellipsis,
    },
};

const GAP: &str = "   ";
const MIN_NAME_WIDTH: usize = 8;

fn trailing_fields(entry: &Entry, now: &DateTime<Local>) -> Vec<String> {
    let mut fields = Vec::new();
    if !entry.is_dir {
        fields.push(size_label(entry.size));
    }
    fields.extend(permissions_string(entry));
    let modified = format_modified_with_time(entry.modified, now);
    if !modified.is_empty() {
        fields.push(modified);
    }
    fields
}

pub fn render_info_line(frame: &mut Frame, area: Rect, entry: Option<&Entry>, now: &DateTime<Local>, theme: &Theme) {
    let Some(entry) = entry else { return };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let name = printable(&entry.name);
    let mut fields = trailing_fields(entry, now);
    let rest_width = |fields: &[String]| fields.iter().map(|field| GAP.len() + field.width()).sum::<usize>();
    let needed_for_name = name.width().min(MIN_NAME_WIDTH);
    while !fields.is_empty() && rest_width(&fields) + needed_for_name > width {
        fields.pop();
    }

    let name = truncate_with_ellipsis(&name, width - rest_width(&fields));
    let mut spans = vec![Span::styled(name, theme.text())];
    for field in fields {
        spans.push(Span::styled(format!("{GAP}{field}"), theme.dim()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests;

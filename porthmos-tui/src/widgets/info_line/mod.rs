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

fn ownership_label(entry: &Entry) -> Option<String> {
    let owner = entry.owner.as_deref().map(printable);
    let group = entry.group.as_deref().map(printable);
    match (owner, group) {
        (Some(owner), Some(group)) => Some(format!("{owner}:{group}")),
        (Some(owner), None) => Some(owner),
        (None, Some(group)) => Some(format!(":{group}")),
        (None, None) => None,
    }
}

fn shown_name(entry: &Entry) -> String {
    let name = printable(&entry.name);
    match &entry.link_target {
        Some(target) if target.is_empty() => format!("{name}@"),
        Some(target) => format!("{name}@ \u{2192} {}", printable(target)),
        None => name,
    }
}

struct Field {
    text: String,
    optional: bool,
}

fn trailing_fields(entry: &Entry, now: &DateTime<Local>) -> Vec<Field> {
    let required = |text: String| Field { text, optional: false };
    let mut fields = Vec::new();
    if !entry.is_dir {
        fields.push(required(size_label(entry.size)));
    }
    fields.extend(permissions_string(entry).map(required));
    fields.extend(ownership_label(entry).map(|text| Field { text, optional: true }));
    let modified = format_modified_with_time(entry.modified, now);
    if !modified.is_empty() {
        fields.push(required(modified));
    }
    fields
}

fn drop_least_important(fields: &mut Vec<Field>) {
    match fields.iter().position(|field| field.optional) {
        Some(position) => fields.remove(position),
        None => fields.pop().expect("a non-empty list has a last field"),
    };
}

pub fn render_info_line(frame: &mut Frame, area: Rect, entry: Option<&Entry>, now: &DateTime<Local>, theme: &Theme) {
    let Some(entry) = entry else { return };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let name = shown_name(entry);
    let mut fields = trailing_fields(entry, now);
    let rest_width = |fields: &[Field]| fields.iter().map(|field| GAP.len() + field.text.width()).sum::<usize>();
    let needed_for_name = name.width().min(MIN_NAME_WIDTH);
    while !fields.is_empty() && rest_width(&fields) + needed_for_name > width {
        drop_least_important(&mut fields);
    }

    let name = truncate_with_ellipsis(&name, width - rest_width(&fields));
    let mut spans = vec![Span::styled(name, theme.text())];
    for field in fields {
        spans.push(Span::styled(format!("{GAP}{}", field.text), theme.dim()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests;

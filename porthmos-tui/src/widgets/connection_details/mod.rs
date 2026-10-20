use porthmos_core::{
    OptionKind, ProtocolInfo,
    profiles::{ConnectionEntry, ConnectionSource},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        connections_view::Selection, frame::hairline, history_view::printable, text_fit::truncate_with_ellipsis,
    },
};

const KEY_WIDTH: usize = 12;
const SAVED: &str = "saved";

pub struct DetailsContext<'a> {
    pub protocols: &'a [ProtocolInfo],
    pub connected: bool,
    pub failure: Option<&'a str>,
}

fn protocol_info<'a>(entry: &ConnectionEntry, protocols: &'a [ProtocolInfo]) -> Option<&'a ProtocolInfo> {
    protocols.iter().find(|info| info.id == entry.protocol)
}

fn protocol_label<'a>(entry: &'a ConnectionEntry, protocols: &'a [ProtocolInfo]) -> &'a str {
    protocol_info(entry, protocols).map_or(entry.protocol.as_str(), |info| info.display_name)
}

pub fn details_lines(entry: &ConnectionEntry, protocols: &[ProtocolInfo]) -> Vec<(String, String)> {
    let info = protocol_info(entry, protocols);
    let mut lines: Vec<(String, String)> = Vec::new();
    let mut push = |key: &str, value: String| {
        if !value.is_empty() {
            lines.push((key.to_lowercase(), printable(&value)));
        }
    };

    push(info.map_or("host", |info| info.form.host.label), entry.host.clone());
    if info.is_none_or(|info| info.form.port.default != entry.port) {
        push("port", entry.port.to_string());
    }
    push(info.map_or("username", |info| info.form.username.label), entry.username.clone());
    if entry.saved_password {
        push("password", SAVED.to_string());
    }

    match info {
        Some(info) => {
            for field in &info.form.options {
                match &field.kind {
                    OptionKind::Secret => {
                        if entry.in_keyring.iter().any(|saved| saved == field.key) {
                            push(field.label, SAVED.to_string());
                        }
                    }
                    OptionKind::Toggle { .. } => {
                        if let Some(value) = entry.option(field.key) {
                            push(field.label, if value == "true" { "yes" } else { "no" }.to_string());
                        }
                    }
                    OptionKind::Text { .. } | OptionKind::Choice { .. } => {
                        push(field.label, entry.option(field.key).unwrap_or_default().to_string());
                    }
                }
            }
        }
        None => {
            for (key, value) in entry.options.iter().filter(|(key, _)| key.as_str() != "remote_path") {
                if entry.in_keyring.iter().any(|saved| saved == key) {
                    push(key, SAVED.to_string());
                } else {
                    push(key, value.clone());
                }
            }
        }
    }

    push("start in", entry.option("remote_path").unwrap_or_default().to_string());
    push("group", entry.group.clone().unwrap_or_default());
    push("tags", entry.tags.join(", "));
    if entry.source == ConnectionSource::SshConfig {
        push("source", "~/.ssh/config".to_string());
    }
    lines
}

fn header_line(left: Vec<Span<'static>>, right: Option<Span<'static>>, width: usize) -> Line<'static> {
    let left_width: usize = left.iter().map(|span| span.content.width()).sum();
    let mut spans = left;
    if let Some(right) = right {
        let right_width = right.content.width();
        if left_width + 2 + right_width <= width {
            spans.push(Span::raw(" ".repeat(width - left_width - right_width)));
            spans.push(right);
        }
    }
    Line::from(spans)
}

fn connection_header(
    entry: &ConnectionEntry, context: &DetailsContext<'_>, width: usize, theme: &Theme,
) -> Line<'static> {
    let right = if let Some(failure) = context.failure {
        let text = truncate_with_ellipsis(&printable(failure.lines().next().unwrap_or("")), width / 2);
        Some(Span::styled(format!("\u{2022} {text}"), theme.err_text()))
    } else if context.connected {
        Some(Span::styled("\u{2022} connected", theme.ok_text()))
    } else {
        None
    };
    let right_width = right.as_ref().map_or(0, |span| span.content.width());
    let protocol = protocol_label(entry, context.protocols).to_string();
    let reserved = if right_width > 0 { right_width + 2 } else { 0 };
    let name_budget = width.saturating_sub(reserved);
    let name = truncate_with_ellipsis(&printable(&entry.name), name_budget);

    let mut left = vec![Span::styled(name.clone(), theme.directory_name())];
    if name.width() + 1 + protocol.width() + reserved <= width {
        left.push(Span::styled(format!(" {protocol}"), theme.dim()));
    }
    header_line(left, right, width)
}

pub fn render_connection_details(
    frame: &mut Frame, area: Rect, selection: Option<Selection<'_>>, group_count: usize, context: &DetailsContext<'_>,
    theme: &Theme,
) {
    let Some(selection) = selection else { return };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let header = match &selection {
        Selection::Connection(entry) => connection_header(entry, context, width, theme),
        Selection::Group(path) => {
            let label =
                if group_count == 1 { "1 connection".to_string() } else { format!("{group_count} connections") };
            let name = truncate_with_ellipsis(&printable(path), width.saturating_sub(label.width() + 2));
            header_line(vec![Span::styled(name, theme.directory_name())], Some(Span::styled(label, theme.dim())), width)
        }
    };
    frame.render_widget(Paragraph::new(header), Rect::new(area.x, area.y, area.width, 1));
    if area.height < 2 {
        return;
    }
    hairline(frame, Rect::new(area.x, area.y + 1, area.width, 1), theme);

    let Selection::Connection(entry) = selection else { return };
    let mut y = area.y + 2;
    for (key, value) in details_lines(entry, context.protocols) {
        if y >= area.bottom() {
            return;
        }
        let key = truncate_with_ellipsis(&key, KEY_WIDTH - 1);
        let padded_key = format!("{key}{}", " ".repeat(KEY_WIDTH - key.width()));
        let value = truncate_with_ellipsis(&value, width.saturating_sub(KEY_WIDTH));
        let line = Line::from(vec![Span::styled(padded_key, theme.dim()), Span::styled(value, theme.text())]);
        frame.render_widget(Paragraph::new(line), Rect::new(area.x, y, area.width, 1));
        y += 1;
    }

    let note = protocol_info(entry, context.protocols).and_then(|info| info.limits_note);
    if let Some(note) = note
        && y + 1 < area.bottom()
    {
        let note_area = Rect::new(area.x, y + 1, area.width, area.bottom() - y - 1);
        frame.render_widget(Paragraph::new(Span::styled(note, theme.dim())).wrap(Wrap { trim: true }), note_area);
    }
}

#[cfg(test)]
mod tests;

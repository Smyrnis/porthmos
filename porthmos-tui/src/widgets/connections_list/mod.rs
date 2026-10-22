use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Utc};

use porthmos_core::{
    ProtocolInfo,
    connections_tree::TreeRow,
    profiles::{ConnectionEntry, ConnectionSource},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{
        connections_view::ConnectionsView,
        filter_line::{FilterLine, FilterLineState, render_filter_line},
        frame::{accent_underline, hairline},
        history_view::printable,
        last_used::last_used_text,
        text_fit::{pad_left, pad_right, truncate_with_ellipsis},
    },
};

const MARKER_WIDTH: usize = 2;
const COLUMN_GAP: usize = 2;
const MIN_NAME_WIDTH: usize = 8;
const MAX_NAME_WIDTH: usize = 18;
const MIN_PROTOCOL_WIDTH: usize = 3;
const MAX_PROTOCOL_WIDTH: usize = 10;
const MIN_DETAIL_WIDTH: usize = 14;
const DOT: &str = "\u{2022}";

pub struct ConnectionsContext<'a> {
    pub connected: &'a HashSet<&'a str>,
    pub failures: &'a HashMap<String, String>,
    pub protocols: &'a [ProtocolInfo],
    pub last_used: &'a BTreeMap<String, DateTime<Utc>>,
    pub now: DateTime<Utc>,
}

struct Columns {
    name: usize,
    protocol: usize,
    age: usize,
    show_age: bool,
}

pub fn render_connections_list(
    frame: &mut Frame, area: Rect, view: &ConnectionsView, context: &ConnectionsContext<'_>, theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (matched, total) = view.match_counts();
    let state = FilterLineState { text: view.filter(), editing: view.editing_filter(), matched, total };
    render_filter_line(frame, Rect::new(area.x, area.y, area.width, 1), state, theme);
    if area.height < 2 {
        return;
    }
    let rule = Rect::new(area.x, area.y + 1, area.width, 1);
    hairline(frame, rule, theme);
    accent_underline(frame, rule, 0..area.width, theme);
    if area.height < 3 {
        return;
    }

    let body = Rect::new(area.x, area.y + 2, area.width, area.height - 2);
    let entries = view.entries();
    let width = usize::from(body.width);
    let columns = columns_for(entries, context, width, deepest_indent(view));

    let mut items: Vec<ListItem> = Vec::new();
    let mut display_cursor = 0;
    for (index, row) in view.rows().iter().enumerate() {
        if let TreeRow::Group { depth: 0, .. } = row
            && index > 0
        {
            items.push(ListItem::new(Line::default()));
        }
        if index == view.cursor {
            display_cursor = items.len();
        }
        let focused = index == view.cursor;
        let line = match row {
            TreeRow::Group { name, depth, count, expanded, .. } => {
                group_line(name, *depth, *count, *expanded, width, theme)
            }
            TreeRow::Connection { index: entry_index, depth } => {
                connection_line(&entries[*entry_index], *depth, focused, &columns, context, width, theme)
            }
        };
        items.push(ListItem::new(line));
    }

    let list = List::new(items).highlight_style(theme.focus_row());
    let mut state = ListState::default();
    if !view.rows().is_empty() {
        state.select(Some(display_cursor));
    }
    frame.render_stateful_widget(list, body, &mut state);
}

fn is_hidden_label(entry: &ConnectionEntry) -> Option<&'static str> {
    match entry.source {
        ConnectionSource::MissingSshHost => Some("not in ~/.ssh/config"),
        ConnectionSource::ShadowedSshHost => Some("hidden by a saved connection"),
        ConnectionSource::Profile | ConnectionSource::SshConfig => None,
    }
}

fn row_indent(depth: usize) -> usize {
    2 * depth.saturating_sub(1)
}

fn deepest_indent(view: &ConnectionsView) -> usize {
    view.rows()
        .iter()
        .filter_map(|row| match row {
            TreeRow::Connection { depth, .. } => Some(row_indent(*depth)),
            TreeRow::Group { .. } => None,
        })
        .max()
        .unwrap_or(0)
}

fn age_text(entry: &ConnectionEntry, context: &ConnectionsContext<'_>) -> String {
    last_used_text(context.now, context.last_used.get(&entry.name).copied())
}

fn columns_for(
    entries: &[ConnectionEntry], context: &ConnectionsContext<'_>, width: usize, deepest_indent: usize,
) -> Columns {
    let name = entries.iter().map(|entry| printable(&entry.name).width()).max().unwrap_or(0);
    let protocol = entries.iter().map(|entry| protocol_label(entry, context.protocols).width()).max().unwrap_or(0);
    let age = entries
        .iter()
        .filter(|entry| is_hidden_label(entry).is_none())
        .map(|entry| age_text(entry, context).width())
        .max()
        .unwrap_or(0);
    let name = name.clamp(MIN_NAME_WIDTH, MAX_NAME_WIDTH);
    let protocol = protocol.clamp(MIN_PROTOCOL_WIDTH, MAX_PROTOCOL_WIDTH);
    let widest_fixed = MARKER_WIDTH + deepest_indent + 2 + name + COLUMN_GAP + protocol + COLUMN_GAP;
    Columns { name, protocol, age, show_age: age > 0 && width >= widest_fixed + MIN_DETAIL_WIDTH + COLUMN_GAP + age }
}

fn group_line(name: &str, depth: usize, count: usize, expanded: bool, width: usize, theme: &Theme) -> Line<'static> {
    let glyph = if expanded { "\u{25be}" } else { "\u{25b8}" };
    let suffix = if expanded { String::new() } else { format!(" ({count})") };
    let indent = " ".repeat(2 * depth);
    let fixed = indent.width() + glyph.width() + 1 + suffix.width();
    let name = truncate_with_ellipsis(&printable(name), width.saturating_sub(fixed));
    Line::from(Span::styled(format!("{indent}{glyph} {name}{suffix}"), theme.dim()))
}

fn connection_line(
    entry: &ConnectionEntry, depth: usize, focused: bool, columns: &Columns, context: &ConnectionsContext<'_>,
    width: usize, theme: &Theme,
) -> Line<'static> {
    let marker = if focused { Span::styled("\u{203a} ", theme.accent_text()) } else { Span::raw("  ") };
    let indent = Span::raw(" ".repeat(row_indent(depth)));
    let name = printable(&entry.name);

    if let Some(reason) = is_hidden_label(entry) {
        let fixed = MARKER_WIDTH + indent.width() + 2 + COLUMN_GAP;
        let budget = width.saturating_sub(fixed);
        let name = truncate_with_ellipsis(&name, budget.min(columns.name.max(MIN_NAME_WIDTH)));
        let reason = truncate_with_ellipsis(reason, budget.saturating_sub(name.width()));
        let name_style = if focused { theme.focused_name() } else { theme.text() };
        return Line::from(vec![
            marker,
            indent,
            Span::styled("!", theme.bright()),
            Span::raw(" "),
            Span::styled(name, name_style),
            Span::raw(" ".repeat(COLUMN_GAP)),
            Span::styled(reason, theme.dim()),
        ]);
    }

    let connected = context.connected.contains(entry.name.as_str());
    let failure = context.failures.get(&entry.name);
    let dot_style = if connected {
        theme.ok_text()
    } else if failure.is_some() {
        theme.err_text()
    } else {
        theme.idle_text()
    };
    let name_style = if focused {
        theme.focused_name()
    } else if connected {
        theme.bright()
    } else {
        theme.text()
    };

    let protocol = pad_right(
        &truncate_with_ellipsis(&protocol_label(entry, context.protocols), columns.protocol),
        columns.protocol,
    );
    let fixed = MARKER_WIDTH + indent.width() + 2 + columns.name + COLUMN_GAP + columns.protocol + COLUMN_GAP;
    let (detail, detail_style) = match failure {
        Some(message) => (printable(message.lines().next().unwrap_or("")), theme.err_text()),
        None => (host_text(entry, context.protocols), theme.dim()),
    };
    let age_width = if columns.show_age { COLUMN_GAP + columns.age } else { 0 };
    let detail_budget = width.saturating_sub(fixed + age_width);
    let detail = truncate_with_ellipsis(&detail, detail_budget);
    let name = pad_right(&truncate_with_ellipsis(&name, columns.name), columns.name);
    let (detail, age_spans) = if columns.show_age {
        let age = pad_left(&age_text(entry, context), columns.age);
        (pad_right(&detail, detail_budget), vec![Span::raw(" ".repeat(COLUMN_GAP)), Span::styled(age, theme.dim())])
    } else {
        (detail, Vec::new())
    };

    let mut spans = vec![
        marker,
        indent,
        Span::styled(DOT, dot_style),
        Span::raw(" "),
        Span::styled(name, name_style),
        Span::raw(" ".repeat(COLUMN_GAP)),
        Span::styled(protocol, theme.dim()),
        Span::raw(" ".repeat(COLUMN_GAP)),
        Span::styled(detail, detail_style),
    ];
    spans.extend(age_spans);
    Line::from(spans)
}

fn host_text(entry: &ConnectionEntry, protocols: &[ProtocolInfo]) -> String {
    let default_port = protocols.iter().find(|info| info.id == entry.protocol).map(|info| info.form.port.default);
    let mut text = if entry.username.is_empty() {
        printable(&entry.host)
    } else {
        format!("{}@{}", printable(&entry.username), printable(&entry.host))
    };
    if default_port.is_some_and(|port| port != entry.port) {
        text.push_str(&format!(":{}", entry.port));
    }
    text
}

fn protocol_label(entry: &ConnectionEntry, protocols: &[ProtocolInfo]) -> String {
    printable(
        protocols
            .iter()
            .find(|info| info.id == entry.protocol)
            .map_or(entry.protocol.as_str(), |info| info.display_name),
    )
}

#[cfg(test)]
mod tests;

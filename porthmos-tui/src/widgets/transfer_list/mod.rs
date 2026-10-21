pub mod advice;

use porthmos_core::transfer::{
    Direction,
    rows::{QueueRow, RowKind, RowState, percent_of},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use self::advice::advice_for;
use crate::{
    rate::{format_eta, format_speed},
    theme::Theme,
    widgets::{
        frame::{accent_underline, hairline},
        history_view::printable,
        text_fit::{pad_left, pad_right, truncate_with_ellipsis},
    },
};

const MARKER_WIDTH: usize = 2;
const ARROW_WIDTH: usize = 2;
const GAP: usize = 2;
const BAR_WIDTH: usize = 16;
const BAR_CELL: &str = "\u{2501}";
const PERCENT_WIDTH: usize = 4;
const SPEED_WIDTH: usize = 10;
const ETA_WIDTH: usize = 7;
const MAX_ROUTE_WIDTH: usize = 24;
const MIN_ITEM_WIDTH: usize = 12;
const MIN_COUNTS_NAME_WIDTH: usize = 4;
const DETAIL_HEIGHT: u16 = 4;
const MIN_HEIGHT_FOR_DETAIL: u16 = 8;

pub struct TransferView<'a> {
    pub rows: &'a [QueueRow],
    pub cursor: usize,
    pub copy_key: Option<&'a str>,
    pub routes: &'a [String],
    pub speeds: &'a [Option<u64>],
    pub etas: &'a [Option<u64>],
    pub failed_time: Option<&'a str>,
}

struct Columns {
    item: usize,
    route: Option<usize>,
    bar: bool,
    percent: bool,
    speed: bool,
    eta: bool,
    status: usize,
}

fn status_of(state: RowState, theme: &Theme) -> (String, Style) {
    match state {
        RowState::Running => ("active".to_string(), theme.bright()),
        RowState::Queued => ("queued".to_string(), theme.dim()),
        RowState::Done => ("done".to_string(), theme.ok_text()),
        RowState::Failed => ("failed".to_string(), theme.err_text()),
        RowState::PartlyFailed(failed) => (format!("done, {failed} failed"), theme.err_text()),
        RowState::Cancelled => ("cancelled".to_string(), theme.dim()),
        RowState::Scanning => ("scanning\u{2026}".to_string(), theme.muted()),
        RowState::AwaitingAnswer => ("waiting for you".to_string(), theme.bright()),
    }
}

fn percent_label(row: &QueueRow) -> Option<String> {
    match row.kind {
        RowKind::Scan(_) => None,
        _ if row.state == RowState::AwaitingAnswer => None,
        _ => Some(format!("{}%", percent_of(row.bytes_done, row.bytes_total).min(100))),
    }
}

fn bar_fill_style(state: RowState, theme: &Theme) -> Option<Style> {
    match state {
        RowState::Running => Some(theme.accent_text()),
        RowState::Done => Some(theme.ok_dim_text()),
        RowState::Failed | RowState::PartlyFailed(_) => Some(theme.err_text()),
        _ => None,
    }
}

fn has_bar(row: &QueueRow) -> bool {
    !matches!(row.kind, RowKind::Scan(_)) && row.state != RowState::AwaitingAnswer
}

fn columns_for(view: &TransferView<'_>, theme: &Theme, width: usize) -> Columns {
    let route_width = view
        .routes
        .iter()
        .map(|route| printable(route).width())
        .max()
        .unwrap_or(0)
        .clamp("route".len(), MAX_ROUTE_WIDTH);
    let status =
        view.rows.iter().map(|row| status_of(row.state, theme).0.width()).max().unwrap_or(0).max("status".len());
    let base = MARKER_WIDTH + ARROW_WIDTH + GAP + status;

    for dropped in 0..=5usize {
        let eta = dropped < 1;
        let speed = dropped < 2;
        let route = dropped < 3;
        let bar = dropped < 4;
        let percent = dropped < 5;
        let fixed = base
            + if eta { GAP + ETA_WIDTH } else { 0 }
            + if speed { GAP + SPEED_WIDTH } else { 0 }
            + if route { GAP + route_width } else { 0 }
            + if bar { GAP + BAR_WIDTH } else { 0 }
            + if percent { 1 + PERCENT_WIDTH } else { 0 };
        if width >= fixed + MIN_ITEM_WIDTH || dropped == 5 {
            return Columns {
                item: width.saturating_sub(fixed).max(1),
                route: route.then_some(route_width),
                bar,
                percent,
                speed,
                eta,
                status,
            };
        }
    }
    unreachable!("the loop returns at its last level")
}

fn header_line(columns: &Columns, theme: &Theme) -> Line<'static> {
    let mut text = " ".repeat(MARKER_WIDTH + ARROW_WIDTH);
    text.push_str(&pad_right("item", columns.item));
    if let Some(route) = columns.route {
        text.push_str(&format!("{}{}", " ".repeat(GAP), pad_right("route", route)));
    }
    if columns.bar {
        text.push_str(&format!("{}{}", " ".repeat(GAP), pad_right("progress", BAR_WIDTH)));
    }
    if columns.percent {
        text.push_str(&" ".repeat(1 + PERCENT_WIDTH));
    }
    if columns.speed {
        text.push_str(&format!("{}{}", " ".repeat(GAP), pad_left("speed", SPEED_WIDTH)));
    }
    if columns.eta {
        text.push_str(&format!("{}{}", " ".repeat(GAP), pad_left("eta", ETA_WIDTH)));
    }
    text.push_str(&format!("{}{}", " ".repeat(GAP), "status"));
    Line::from(Span::styled(text, theme.dim()))
}

fn row_line(view: &TransferView<'_>, index: usize, focused: bool, columns: &Columns, theme: &Theme) -> Line<'static> {
    let row = &view.rows[index];
    let marker = if focused { Span::styled("\u{203a}", theme.accent_text()) } else { Span::raw(" ") };
    let arrow = match row.direction {
        Direction::Upload => "\u{2191}",
        Direction::Download => "\u{2193}",
    };
    let name_style = if focused { theme.focused_name() } else { theme.text() };
    let counts = matches!(row.kind, RowKind::Batch(_)).then(|| format!(" {} of {}", row.files_done, row.files_total));
    let counts = counts.filter(|counts| counts.width() + MIN_COUNTS_NAME_WIDTH <= columns.item);
    let counts_width = counts.as_ref().map_or(0, |counts| counts.width());
    let name = truncate_with_ellipsis(&printable(&row.label), columns.item - counts_width);
    let padding = " ".repeat(columns.item.saturating_sub(name.width() + counts_width));

    let mut spans = vec![marker, Span::raw(" "), Span::styled(arrow, theme.muted()), Span::raw(" ")];
    spans.push(Span::styled(name, name_style));
    if let Some(counts) = counts {
        spans.push(Span::styled(counts, theme.dim()));
    }
    spans.push(Span::raw(padding));
    if let Some(route_width) = columns.route {
        let route = view.routes.get(index).map(|route| printable(route)).unwrap_or_default();
        let route = pad_right(&truncate_with_ellipsis(&route, route_width), route_width);
        spans.push(Span::styled(format!("{}{route}", " ".repeat(GAP)), theme.dim()));
    }
    if columns.bar {
        spans.push(Span::raw(" ".repeat(GAP)));
        spans.extend(bar_spans(row, theme));
    }
    if columns.percent {
        let percent = percent_label(row).unwrap_or_default();
        spans.push(Span::styled(format!(" {}", pad_left(&percent, PERCENT_WIDTH)), theme.muted()));
    }
    if columns.speed {
        let speed = view.speeds.get(index).copied().flatten().map(format_speed).unwrap_or_default();
        spans.push(Span::styled(format!("{}{}", " ".repeat(GAP), pad_left(&speed, SPEED_WIDTH)), theme.dim()));
    }
    if columns.eta {
        let eta = view.etas.get(index).copied().flatten().map(format_eta).unwrap_or_default();
        spans.push(Span::styled(format!("{}{}", " ".repeat(GAP), pad_left(&eta, ETA_WIDTH)), theme.dim()));
    }
    let (status, status_style) = status_of(row.state, theme);
    spans.push(Span::styled(format!("{}{}", " ".repeat(GAP), pad_right(&status, columns.status)), status_style));
    Line::from(spans)
}

fn bar_spans(row: &QueueRow, theme: &Theme) -> Vec<Span<'static>> {
    if !has_bar(row) {
        return vec![Span::raw(" ".repeat(BAR_WIDTH))];
    }
    let percent = usize::from(percent_of(row.bytes_done, row.bytes_total).min(100));
    let fill_style = bar_fill_style(row.state, theme);
    let filled = if fill_style.is_some() { ((percent * BAR_WIDTH + 50) / 100).min(BAR_WIDTH) } else { 0 };
    vec![
        Span::styled(BAR_CELL.repeat(filled), fill_style.unwrap_or_default()),
        Span::styled(BAR_CELL.repeat(BAR_WIDTH - filled), theme.line_strong_text()),
    ]
}

fn error_part(message: &str) -> &str {
    message.split_once('\n').map_or(message, |(_, error)| error)
}

fn detail_lines(row: &QueueRow, failed_time: Option<&str>, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let Some(failure) = &row.failure else { return Vec::new() };
    let error = error_part(&failure.message);
    let time = failed_time.map(|time| format!(" \u{b7} {time}")).unwrap_or_default();
    let outcome = match failure.attempts {
        0 => "did not start".to_string(),
        1 => "failed after 1 attempt".to_string(),
        attempts => format!("failed after {attempts} attempts"),
    };
    let suffix = format!("  {outcome}{time}");
    let file = truncate_with_ellipsis(&printable(&failure.file), width.saturating_sub(2 + suffix.width()).max(1));
    let mut lines = vec![Line::from(vec![
        Span::styled("\u{2022} ", theme.err_text()),
        Span::styled(file, theme.text()),
        Span::styled(suffix, theme.muted()),
    ])];
    lines.push(Line::from(Span::styled(truncate_with_ellipsis(&printable(error), width), theme.muted())));
    if let Some(advice) = advice_for(error) {
        lines.push(Line::from(Span::styled(truncate_with_ellipsis(advice, width), theme.dim())));
    }
    lines
}

pub fn render_transfer_list(frame: &mut Frame, area: Rect, view: &TransferView<'_>, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if view.rows.is_empty() {
        let message = match view.copy_key {
            Some(key) => format!("No transfers \u{2014} {key} copies the selected file or folder"),
            None => "No transfers yet".to_string(),
        };
        let text = truncate_with_ellipsis(&message, usize::from(area.width));
        frame.render_widget(Paragraph::new(Span::styled(text, theme.dim())), Rect::new(area.x, area.y, area.width, 1));
        return;
    }

    let width = usize::from(area.width);
    let columns = columns_for(view, theme, width);
    frame.render_widget(Paragraph::new(header_line(&columns, theme)), Rect::new(area.x, area.y, area.width, 1));
    if area.height < 2 {
        return;
    }
    let rule = Rect::new(area.x, area.y + 1, area.width, 1);
    hairline(frame, rule, theme);
    accent_underline(frame, rule, 0..area.width, theme);

    let cursor = view.cursor.min(view.rows.len() - 1);
    let focused_row = &view.rows[cursor];
    let detail = if area.height >= MIN_HEIGHT_FOR_DETAIL
        && matches!(focused_row.state, RowState::Failed | RowState::PartlyFailed(_))
    {
        detail_lines(focused_row, view.failed_time, width, theme)
    } else {
        Vec::new()
    };
    let detail_height = if detail.is_empty() { 0 } else { DETAIL_HEIGHT };

    let list_height = area.height - 2 - detail_height;
    if list_height > 0 {
        let items: Vec<ListItem> = (0..view.rows.len())
            .map(|index| ListItem::new(row_line(view, index, index == cursor, &columns, theme)))
            .collect();
        let list = List::new(items).highlight_style(theme.focus_row());
        let mut state = ListState::default();
        state.select(Some(cursor));
        frame.render_stateful_widget(list, Rect::new(area.x, area.y + 2, area.width, list_height), &mut state);
    }
    if detail_height > 0 {
        let top = area.y + area.height - DETAIL_HEIGHT;
        hairline(frame, Rect::new(area.x, top, area.width, 1), theme);
        frame.render_widget(Paragraph::new(detail), Rect::new(area.x, top + 1, area.width, DETAIL_HEIGHT - 1));
    }
}

pub fn transfer_summary(rows: &[QueueRow], speed: Option<u64>) -> String {
    let active = rows.iter().filter(|row| row.state == RowState::Running).count();
    let queued = rows.iter().filter(|row| row.state == RowState::Queued).count();
    let failed = rows.iter().filter(|row| matches!(row.state, RowState::Failed | RowState::PartlyFailed(_))).count();

    let mut parts = Vec::new();
    for (count, label) in [(active, "active"), (queued, "queued"), (failed, "failed")] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    if !parts.is_empty() || speed.is_some() {
        parts.extend(speed.map(format_speed));
    }
    parts.join(" \u{b7} ")
}

#[cfg(test)]
mod tests;

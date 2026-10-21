use std::collections::HashMap;

use porthmos_core::transfer::{
    Direction,
    rows::{QueueRow, RowState, percent_of},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    rate::{RateTracker, format_eta, format_speed},
    theme::Theme,
    widgets::{history_view::printable, text_fit::truncate_with_ellipsis},
};

const BAR_WIDTH: usize = 22;
const BAR_CELL: &str = "\u{2501}";
const SECTION_GAP: &str = "  ";
const QUEUE_GAP: usize = 3;
const STATS_SEPARATOR: &str = " \u{b7} ";
const DROP_ORDER: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineData {
    pub arrow: String,
    pub label: String,
    pub target: Option<String>,
    pub percent: u8,
    pub counts: Option<(usize, usize)>,
    pub speed: Option<u64>,
    pub eta: Option<u64>,
    pub queued: usize,
    pub incoming: bool,
    pub waiting: bool,
}

fn arrow_of(rows: &[&QueueRow]) -> String {
    let uploads = rows.iter().filter(|row| row.direction == Direction::Upload).count();
    if uploads == rows.len() {
        "\u{2191}".to_string()
    } else if uploads == 0 {
        "\u{2193}".to_string()
    } else {
        "\u{2191}\u{2193}".to_string()
    }
}

fn percent(done: u64, total: u64) -> u8 {
    percent_of(done, total).min(100)
}

pub fn line_data(rows: &[QueueRow], names: &HashMap<u64, String>, rates: &RateTracker) -> Option<LineData> {
    let running: Vec<&QueueRow> = rows.iter().filter(|row| row.state == RowState::Running).collect();
    let queued_rows = rows.iter().filter(|row| row.state == RowState::Queued).count();
    let waiting = running.is_empty();
    let (shown, queued): (Vec<&QueueRow>, usize) = if running.is_empty() {
        (rows.iter().filter(|row| row.state == RowState::Queued).take(1).collect(), queued_rows.saturating_sub(1))
    } else {
        (running, queued_rows)
    };

    match shown.as_slice() {
        [] => None,
        [row] => Some(LineData {
            arrow: arrow_of(&shown),
            label: row.label.clone(),
            target: row.session_id.map(|session| names.get(&session).cloned().unwrap_or_else(|| "remote".to_string())),
            percent: percent(row.bytes_done, row.bytes_total),
            counts: matches!(row.kind, porthmos_core::transfer::rows::RowKind::Batch(_))
                .then_some((row.files_done, row.files_total)),
            speed: rates.speed(row.kind),
            eta: rates.eta_seconds(row),
            queued,
            incoming: row.direction == Direction::Download,
            waiting,
        }),
        many => {
            let done = many.iter().fold(0u64, |sum, row| sum.saturating_add(row.bytes_done));
            let total = many.iter().fold(0u64, |sum, row| sum.saturating_add(row.bytes_total));
            let speed = rates.aggregate_speed();
            let eta = speed.filter(|speed| *speed > 0 && total > 0).map(|speed| total.saturating_sub(done) / speed);
            Some(LineData {
                arrow: arrow_of(many),
                label: format!("{} transfers", many.len()),
                target: None,
                percent: percent(done, total),
                counts: None,
                speed,
                eta,
                queued,
                incoming: false,
                waiting: false,
            })
        }
    }
}

struct Shown {
    queued: bool,
    eta: bool,
    speed: bool,
    counts: bool,
    bar: bool,
    target: bool,
}

impl Shown {
    fn after_dropping(dropped: usize) -> Self {
        Self {
            queued: dropped < 1,
            eta: dropped < 2,
            speed: dropped < 3,
            counts: dropped < 4,
            bar: dropped < 5,
            target: dropped < 6,
        }
    }
}

fn bar_spans(percent: u8, theme: &Theme) -> [Span<'static>; 2] {
    let filled = ((usize::from(percent) * BAR_WIDTH + 50) / 100).min(BAR_WIDTH);
    [
        Span::styled(BAR_CELL.repeat(filled), theme.accent_text()),
        Span::styled(BAR_CELL.repeat(BAR_WIDTH - filled), theme.line_strong_text()),
    ]
}

fn stats_text(data: &LineData, shown: &Shown) -> String {
    let mut parts = Vec::new();
    if let Some((done, total)) = data.counts.filter(|_| shown.counts) {
        parts.push(format!("{done}/{total}"));
    }
    if let Some(speed) = data.speed.filter(|_| shown.speed) {
        parts.push(format_speed(speed));
    }
    if let Some(eta) = data.eta.filter(|_| shown.eta) {
        parts.push(format_eta(eta));
    }
    parts.join(STATS_SEPARATOR)
}

fn left_spans(data: &LineData, shown: &Shown, label: String, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(format!("{} {label}", data.arrow), theme.bright())];
    if let Some(target) = data.target.as_ref().filter(|_| shown.target) {
        let pointer = if data.incoming { "\u{2190}" } else { "\u{2192}" };
        spans.push(Span::styled(format!("{SECTION_GAP}{pointer} {}", printable(target)), theme.dim()));
    }
    if data.waiting {
        spans.push(Span::styled(format!("{SECTION_GAP}queued"), theme.muted()));
    } else {
        if shown.bar {
            spans.push(Span::raw(SECTION_GAP));
            spans.extend(bar_spans(data.percent, theme));
        }
        spans.push(Span::styled(format!("{SECTION_GAP}{}%", data.percent), theme.bright()));
    }
    let stats = stats_text(data, shown);
    if !stats.is_empty() {
        spans.push(Span::styled(format!("{SECTION_GAP}{stats}"), theme.muted()));
    }
    spans
}

fn width_of(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.width()).sum()
}

pub fn render_transfer_line(frame: &mut Frame, area: Rect, data: &LineData, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let width = usize::from(area.width);
    let label = printable(&data.label);
    let queued_text = (data.queued > 0).then(|| format!("+{} queued", data.queued));

    let mut chosen = None;
    for dropped in 0..=DROP_ORDER {
        let shown = Shown::after_dropping(dropped);
        let spans = left_spans(data, &shown, label.clone(), theme);
        let queue_width = queued_text.as_ref().filter(|_| shown.queued).map_or(0, |text| QUEUE_GAP + text.width());
        if width_of(&spans) + queue_width <= width {
            chosen = Some((shown, spans));
            break;
        }
    }
    let (shown, mut spans) = chosen.unwrap_or_else(|| {
        let shown = Shown::after_dropping(DROP_ORDER);
        let without_label = width_of(&left_spans(data, &shown, String::new(), theme));
        let budget = width.saturating_sub(without_label);
        let spans = left_spans(data, &shown, truncate_with_ellipsis(&label, budget), theme);
        (shown, spans)
    });

    if let Some(text) = queued_text.filter(|_| shown.queued) {
        let padding = width.saturating_sub(width_of(&spans) + text.width());
        spans.push(Span::raw(" ".repeat(padding)));
        spans.push(Span::styled(text, theme.muted()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), Rect::new(area.x, area.y, area.width, 1));
}

#[cfg(test)]
mod tests;

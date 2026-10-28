use std::time::Duration;

use porthmos_core::{NotTrusted, StartFolder, TEST_LIMIT, TestOutcome, TestStage};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    theme::Theme,
    widgets::{dialog::TestState, history_view::joined_lines, text_fit::truncate_with_ellipsis},
};

const SEPARATOR: &str = " \u{b7} ";
const GLYPH_WIDTH: usize = 2;
const MIN_CUT_WIDTH: usize = 2;

struct Segment {
    text: String,
    style: Style,
    cuttable: bool,
}

impl Segment {
    fn fixed(text: impl Into<String>, style: Style) -> Self {
        Self { text: text.into(), style, cuttable: false }
    }

    fn cuttable(text: impl Into<String>, style: Style) -> Self {
        Self { text: text.into(), style, cuttable: true }
    }
}

pub fn format_elapsed(elapsed: Duration) -> String {
    let millis = elapsed.as_millis();
    if millis < 10_000 { format!("{millis} ms") } else { format!("{:.1} s", millis as f64 / 1000.0) }
}

fn clean(text: &str) -> String {
    joined_lines(text)
}

fn stage_phrase(stage: TestStage) -> &'static str {
    match stage {
        TestStage::Settings => "while checking the settings",
        TestStage::Connect => "while connecting",
        TestStage::Login => "while logging in",
        TestStage::StartFolder => "while checking the start folder",
    }
}

fn failure_phrase(stage: TestStage) -> Option<&'static str> {
    match stage {
        TestStage::Settings => None,
        TestStage::Connect => Some("failed to connect"),
        TestStage::Login => Some("failed at login"),
        TestStage::StartFolder => Some("failed at the start folder"),
    }
}

fn folder_segment(folder: &StartFolder, theme: &Theme) -> Segment {
    match folder {
        StartFolder::Opens(path) => Segment::cuttable(format!("{} opens", clean(path)), theme.text()),
        StartFolder::Missing(path) => {
            Segment::cuttable(format!("start folder {} not found", clean(path)), theme.bright())
        }
        StartFolder::Unreadable { path, .. } => {
            Segment::cuttable(format!("{} not readable", clean(path)), theme.bright())
        }
        StartFolder::Unchecked => Segment::cuttable("start folder not checked", theme.dim()),
    }
}

fn describe(state: &TestState, theme: &Theme) -> (&'static str, Style, Vec<Segment>) {
    let outcome = match state {
        TestState::Running => {
            return ("\u{25cb} ", theme.dim(), vec![Segment::fixed("testing\u{2026}", theme.dim())]);
        }
        TestState::Finished(outcome) => outcome,
    };
    match outcome {
        TestOutcome::Connected(report) => {
            let mut segments = vec![
                Segment::fixed("connected", theme.text()),
                Segment::fixed(format_elapsed(report.elapsed), theme.bright()),
            ];
            if let Some(server) = report.server.as_deref().map(clean).filter(|server| !server.is_empty()) {
                segments.push(Segment::cuttable(server, theme.text()));
            }
            segments.push(folder_segment(&report.start_folder, theme));
            ("\u{25cf} ", theme.ok_text(), segments)
        }
        TestOutcome::NotTrusted(untrusted) => {
            let (what, fingerprint) = match untrusted {
                NotTrusted::HostKey { fingerprint, .. } => ("host key not trusted yet", fingerprint),
                NotTrusted::Certificate { fingerprint, .. } => ("certificate not trusted yet", fingerprint),
            };
            let segments = vec![
                Segment::fixed("host reachable", theme.text()),
                Segment::fixed(what, theme.bright()),
                Segment::cuttable(clean(fingerprint), theme.text()),
            ];
            ("\u{25cf} ", theme.bright(), segments)
        }
        TestOutcome::Failed { stage, message } => {
            let mut segments = Vec::new();
            match failure_phrase(*stage) {
                Some(phrase) => {
                    segments.push(Segment::fixed(phrase, theme.err_text()));
                    segments.push(Segment::cuttable(clean(message), theme.text()));
                }
                None => segments.push(Segment::cuttable(clean(message), theme.err_text())),
            }
            ("\u{25cf} ", theme.err_text(), segments)
        }
        TestOutcome::TimedOut(stage) => {
            let text = format!("timed out after {} s {}", TEST_LIMIT.as_secs(), stage_phrase(*stage));
            ("\u{25cf} ", theme.err_text(), vec![Segment::cuttable(text, theme.err_text())])
        }
        TestOutcome::Cancelled => ("\u{25cf} ", theme.dim(), vec![Segment::cuttable("test cancelled", theme.dim())]),
    }
}

fn total_width(segments: &[Segment]) -> usize {
    let texts: usize = segments.iter().map(|segment| segment.text.width()).sum();
    GLYPH_WIDTH + texts + SEPARATOR.width() * segments.len().saturating_sub(1)
}

fn fit(segments: &mut Vec<Segment>, width: usize) {
    for only_cuttable in [true, false] {
        for index in (0..segments.len()).rev() {
            let total = total_width(segments);
            if total <= width {
                return;
            }
            if only_cuttable && !segments[index].cuttable {
                continue;
            }
            let excess = total - width;
            let current = segments[index].text.width();
            if current > excess + MIN_CUT_WIDTH {
                segments[index].text = truncate_with_ellipsis(&segments[index].text, current - excess);
            } else {
                segments.remove(index);
            }
        }
    }
}

pub fn line(state: &TestState, width: usize, theme: &Theme) -> Line<'static> {
    let (glyph, glyph_style, mut segments) = describe(state, theme);
    fit(&mut segments, width);
    let mut spans = vec![Span::styled(glyph, glyph_style)];
    for (index, segment) in segments.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(SEPARATOR, theme.dim()));
        }
        spans.push(Span::styled(segment.text, segment.style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests;

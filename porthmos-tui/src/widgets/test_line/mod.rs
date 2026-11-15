use porthmos_core::i18n;
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

fn stage_phrase(stage: TestStage) -> String {
    i18n::text(match stage {
        TestStage::Settings => "trial.while_settings",
        TestStage::Connect => "trial.while_connecting",
        TestStage::Login => "trial.while_login",
        TestStage::StartFolder => "trial.while_folder",
    })
}

fn failure_phrase(stage: TestStage) -> Option<String> {
    let key = match stage {
        TestStage::Settings => return None,
        TestStage::Connect => "trial.failed_connect",
        TestStage::Login => "trial.failed_login",
        TestStage::StartFolder => "trial.failed_folder",
    };
    Some(i18n::text(key))
}

fn folder_segment(folder: &StartFolder, theme: &Theme) -> Segment {
    match folder {
        StartFolder::Opens(path) => {
            Segment::cuttable(i18n::text_with("trial.opens", &[("path", &clean(path))]), theme.text())
        }
        StartFolder::Missing(path) => {
            Segment::cuttable(i18n::text_with("trial.folder_missing", &[("path", &clean(path))]), theme.bright())
        }
        StartFolder::Unreadable { path, .. } => {
            Segment::cuttable(i18n::text_with("trial.not_readable", &[("path", &clean(path))]), theme.bright())
        }
        StartFolder::Unchecked => Segment::cuttable(i18n::text("trial.folder_unchecked"), theme.dim()),
    }
}

fn describe(state: &TestState, theme: &Theme) -> (&'static str, Style, Vec<Segment>) {
    let outcome = match state {
        TestState::Running => {
            return ("\u{25cb} ", theme.dim(), vec![Segment::fixed(i18n::text("trial.testing"), theme.dim())]);
        }
        TestState::Finished(outcome) => outcome,
    };
    match outcome {
        TestOutcome::Connected(report) => {
            let mut segments = vec![
                Segment::fixed(i18n::text("trial.connected"), theme.text()),
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
                NotTrusted::HostKey { fingerprint, .. } => (i18n::text("trial.host_key_untrusted"), fingerprint),
                NotTrusted::Certificate { fingerprint, .. } => (i18n::text("trial.cert_untrusted"), fingerprint),
            };
            let segments = vec![
                Segment::fixed(i18n::text("trial.host_reachable"), theme.text()),
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
                    segments.push(Segment::cuttable(clean(&message.one_line()), theme.text()));
                }
                None => segments.push(Segment::cuttable(clean(&message.one_line()), theme.err_text())),
            }
            ("\u{25cf} ", theme.err_text(), segments)
        }
        TestOutcome::TimedOut(stage) => {
            let text = i18n::text_with(
                "trial.timed_out",
                &[("seconds", &TEST_LIMIT.as_secs().to_string()), ("stage", &stage_phrase(*stage))],
            );
            ("\u{25cf} ", theme.err_text(), vec![Segment::cuttable(text, theme.err_text())])
        }
        TestOutcome::Cancelled => {
            ("\u{25cf} ", theme.dim(), vec![Segment::cuttable(i18n::text("trial.cancelled"), theme.dim())])
        }
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

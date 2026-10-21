use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::transfer::{conflicts::Resolution, plan::ExistingFile};
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
        file_list::format_size,
        history_view::printable,
        popup::{content_width, footer, footer_text, render_popup},
        text_fit::truncate_with_ellipsis,
    },
};

const LABEL_WIDTH: usize = 10;
const SIZE_WIDTH: usize = 8;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

pub struct ConflictDialog {
    pub file_name: String,
    pub existing: Option<ExistingFile>,
    pub partial: Option<ExistingFile>,
    pub new_size: u64,
    pub new_modified: Option<u64>,
    pub index: usize,
    pub total: usize,
    pub apply_to_rest: bool,
    pub now: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictOutcome {
    Pending,
    Resolved { resolution: Option<Resolution>, apply_to_rest: bool },
}

impl ConflictDialog {
    fn offers_resume(&self) -> bool {
        self.partial.is_some() && !self.existing_is_dir()
    }

    fn existing_is_dir(&self) -> bool {
        self.existing.is_some_and(|existing| existing.is_dir)
    }

    pub fn remaining_after_this(&self) -> usize {
        self.total.saturating_sub(self.index + 1)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ConflictOutcome {
        let apply_to_rest = self.apply_to_rest;
        let answer = |resolution| ConflictOutcome::Resolved { resolution: Some(resolution), apply_to_rest };
        let with_shortcut_modifier = key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('c' | 'C') | KeyCode::Esc => {
                ConflictOutcome::Resolved { resolution: None, apply_to_rest: false }
            }
            _ if with_shortcut_modifier => ConflictOutcome::Pending,
            KeyCode::Char('u' | 'U') if self.offers_resume() => answer(Resolution::Resume),
            KeyCode::Char('o' | 'O') if !self.existing_is_dir() => answer(Resolution::Overwrite),
            KeyCode::Char('s' | 'S') => answer(Resolution::Skip),
            KeyCode::Char('r' | 'R') if self.existing.is_some() => answer(Resolution::Rename),
            KeyCode::Char('a' | 'A') if self.remaining_after_this() > 0 => {
                self.apply_to_rest = !self.apply_to_rest;
                ConflictOutcome::Pending
            }
            _ => ConflictOutcome::Pending,
        }
    }

    fn file_rows(&self) -> Vec<(&'static str, String, String)> {
        let mut rows = Vec::new();
        if let Some(existing) = self.existing {
            rows.push(if existing.is_dir {
                ("existing", "folder".to_string(), String::new())
            } else {
                (
                    "existing",
                    format_size(existing.size, false),
                    format!("modified {}", age_text(existing.modified, self.now)),
                )
            });
        }
        if let Some(partial) = self.partial {
            rows.push((
                "partial",
                format!("{} of {}", format_size(partial.size, false), format_size(self.new_size, false)),
                format!("modified {}", age_text(partial.modified, self.now)),
            ));
        }
        rows.push((
            "new",
            format_size(self.new_size, false),
            format!("modified {}", age_text(self.new_modified, self.now)),
        ));
        rows
    }

    fn option_pairs(&self) -> Vec<(&'static str, &'static str)> {
        let mut options = Vec::new();
        if self.offers_resume() {
            options.push(("u", "resume"));
        }
        if !self.existing_is_dir() {
            options.push(("o", if self.existing.is_some() { "overwrite" } else { "start over" }));
        }
        options.push(("s", "skip"));
        if self.existing.is_some() {
            options.push(("r", "rename"));
        }
        options.push(("c", "cancel copy"));
        options
    }

    fn same_answer_text(&self) -> Option<String> {
        let remaining = self.remaining_after_this();
        (remaining > 0).then(|| format!("same answer for the other {remaining}"))
    }
}

pub fn format_age(seconds_ago: i64) -> String {
    let (count, unit) = if seconds_ago < MINUTE {
        return "just now".to_string();
    } else if seconds_ago < HOUR {
        (seconds_ago / MINUTE, "minute")
    } else if seconds_ago < DAY {
        (seconds_ago / HOUR, "hour")
    } else {
        (seconds_ago / DAY, "day")
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

fn age_text(modified: Option<u64>, now: u64) -> String {
    match modified {
        Some(modified) => format_age(now as i64 - modified as i64),
        None => "unknown".to_string(),
    }
}

fn row_text((label, size, modified): &(&'static str, String, String)) -> String {
    let gap = if modified.is_empty() { "" } else { "   " };
    format!("{label:<LABEL_WIDTH$}{size:>SIZE_WIDTH$}{gap}{modified}")
}

pub fn render_conflict(frame: &mut Frame, area: Rect, dialog: &ConflictDialog, theme: &Theme) {
    let rows = dialog.file_rows();
    let row_lines: Vec<String> = rows.iter().map(row_text).collect();
    let pairs = dialog.option_pairs();
    let option_line = footer_text(&pairs);
    let same_answer = dialog.same_answer_text();

    let state = if dialog.existing.is_some() { "already exists" } else { "was partly copied before" };
    let suffix = format!("\" {state} ({} of {})", dialog.index + 1, dialog.total);
    let mut measured: Vec<&str> = row_lines.iter().map(String::as_str).collect();
    measured.push(&option_line);
    let same_answer_line = same_answer.as_ref().map(|text| format!("a [ ] {text}"));
    measured.extend(same_answer_line.as_deref());
    let width = content_width(&measured).max(content_width(&[&suffix])).min(area.width);
    let name_budget = usize::from(width).saturating_sub(6 + 1 + suffix.width()).max(1);
    let title = format!("\"{}{suffix}", truncate_with_ellipsis(&printable(&dialog.file_name), name_budget));

    let height = u16::try_from(row_lines.len() + 4 + usize::from(same_answer.is_some())).unwrap_or(u16::MAX);
    let inner = render_popup(frame, area, (width, height.min(area.height)), Some(&title), theme);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let file_lines: Vec<Line> = rows
        .iter()
        .map(|(label, size, modified)| {
            let mut spans = vec![
                Span::styled(format!("{label:<LABEL_WIDTH$}"), theme.dim()),
                Span::styled(format!("{size:>SIZE_WIDTH$}"), theme.text()),
            ];
            if !modified.is_empty() {
                spans.push(Span::styled(format!("   {modified}"), theme.dim()));
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(file_lines), Rect::new(inner.x, inner.y, inner.width, inner.height));

    let rows_height = u16::try_from(row_lines.len()).unwrap_or(u16::MAX);
    let options_y = inner.y + rows_height + 1;
    if options_y >= inner.bottom() {
        return;
    }
    footer(frame, Rect::new(inner.x, options_y, inner.width, 1), &pairs, theme);
    if let Some(text) = same_answer.filter(|_| options_y + 1 < inner.bottom()) {
        let checkbox = if dialog.apply_to_rest {
            Span::styled("[x]", theme.accent_text())
        } else {
            Span::styled("[ ]", theme.dim())
        };
        let line = Line::from(vec![
            Span::styled("a", theme.bright()),
            Span::raw(" "),
            checkbox,
            Span::styled(format!(" {text}"), theme.dim()),
        ]);
        frame.render_widget(Paragraph::new(line), Rect::new(inner.x, options_y + 1, inner.width, 1));
    }
}

#[cfg(test)]
mod tests;

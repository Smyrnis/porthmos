use std::time::Duration;

use similar::{Algorithm, ChangeTag, TextDiffConfig};

use crate::preview::{PreviewContent, classify};

pub const CONTEXT_LINES: usize = 3;
pub const MAX_DIFF_ROWS: usize = 5000;
const TAB_AS_SPACES: &str = "    ";
const DIFF_DEADLINE: Duration = Duration::from_millis(500);
const BYTES_PER_MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Context,
    Removed,
    Added,
    Skipped(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

impl DiffLine {
    pub fn marked_text(&self) -> String {
        match self.kind {
            DiffKind::Context => format!("  {}", self.text),
            DiffKind::Removed => format!("- {}", self.text),
            DiffKind::Added => format!("+ {}", self.text),
            DiffKind::Skipped(1) => "\u{2026} 1 unchanged line \u{2026}".to_string(),
            DiffKind::Skipped(count) => format!("\u{2026} {count} unchanged lines \u{2026}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOutcome {
    Changes(Vec<DiffLine>),
    OnlyLineEndings,
    Identical,
    NotText,
    TooLarge { limit: u64 },
    TooManyChanges,
    RemoteGone,
}

impl DiffOutcome {
    pub fn note(&self) -> Option<String> {
        let sentence = match self {
            DiffOutcome::Changes(_) => return None,
            DiffOutcome::OnlyLineEndings => "Only line endings differ.".to_string(),
            DiffOutcome::Identical => "The server's file is now the same as yours.".to_string(),
            DiffOutcome::NotText => "Not a text file, so no comparison is shown.".to_string(),
            DiffOutcome::TooLarge { limit } => format!(
                "Too large to compare (over {} MiB). The limit is [edit] compare_limit_mib.",
                limit / BYTES_PER_MIB
            ),
            DiffOutcome::TooManyChanges => "Too many changes to show.".to_string(),
            DiffOutcome::RemoteGone => "The file no longer exists on the server.".to_string(),
        };
        Some(sentence)
    }
}

fn hides_or_reorders(character: char) -> bool {
    matches!(
        character,
        '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200b}'..='\u{200f}'
            | '\u{061c}'
            | '\u{feff}'
    )
}

fn display_safe(line: &str) -> String {
    line.chars()
        .map(|character| match character {
            '\t' => TAB_AS_SPACES.to_string(),
            unsafe_char if unsafe_char.is_control() || hides_or_reorders(unsafe_char) => "?".to_string(),
            other => other.to_string(),
        })
        .collect()
}

fn text_of(bytes: Vec<u8>) -> Option<String> {
    match classify(bytes, false) {
        PreviewContent::Text(text) => Some(text),
        PreviewContent::Binary(_) => None,
    }
}

struct Rows {
    lines: Vec<DiffLine>,
}

impl Rows {
    fn push(&mut self, kind: DiffKind, text: &str) -> Result<(), DiffOutcome> {
        if self.lines.len() >= MAX_DIFF_ROWS {
            return Err(DiffOutcome::TooManyChanges);
        }
        self.lines.push(DiffLine { kind, text: display_safe(text) });
        Ok(())
    }

    fn skip(&mut self, hidden: usize) -> Result<(), DiffOutcome> {
        if hidden == 0 {
            return Ok(());
        }
        self.push(DiffKind::Skipped(hidden), "")
    }
}

pub fn compute(remote: Vec<u8>, mine: Vec<u8>) -> DiffOutcome {
    let (Some(remote_text), Some(mine_text)) = (text_of(remote), text_of(mine)) else {
        return DiffOutcome::NotText;
    };
    if remote_text == mine_text {
        return DiffOutcome::Identical;
    }
    let remote_lines: Vec<&str> = remote_text.lines().collect();
    let mine_lines: Vec<&str> = mine_text.lines().collect();
    if remote_lines == mine_lines {
        return DiffOutcome::OnlyLineEndings;
    }
    match rows_between(&remote_lines, &mine_lines) {
        Ok(lines) => DiffOutcome::Changes(lines),
        Err(outcome) => outcome,
    }
}

fn rows_between(remote: &[&str], mine: &[&str]) -> Result<Vec<DiffLine>, DiffOutcome> {
    let diff = TextDiffConfig::default().algorithm(Algorithm::Myers).timeout(DIFF_DEADLINE).diff_slices(remote, mine);
    let mut rows = Rows { lines: Vec::new() };
    let mut shown_up_to = 0;
    for group in diff.grouped_ops(CONTEXT_LINES) {
        let start = group.first().map_or(0, |op| op.old_range().start);
        rows.skip(start - shown_up_to)?;
        for op in &group {
            for change in diff.iter_changes(op) {
                let kind = match change.tag() {
                    ChangeTag::Equal => DiffKind::Context,
                    ChangeTag::Delete => DiffKind::Removed,
                    ChangeTag::Insert => DiffKind::Added,
                };
                rows.push(kind, change.value())?;
            }
        }
        shown_up_to = group.last().map_or(shown_up_to, |op| op.old_range().end);
    }
    rows.skip(remote.len() - shown_up_to)?;
    Ok(rows.lines)
}

#[cfg(test)]
mod tests;

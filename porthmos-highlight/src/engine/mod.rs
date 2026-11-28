use std::{
    path::Path,
    sync::OnceLock,
    time::{Duration, Instant},
};

use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

use crate::{Highlighted, Span, TokenKind, kind::kind_of_scopes, lines_of};

pub const MAX_HIGHLIGHT_BYTES: usize = 256 * 1024;
pub const MAX_HIGHLIGHT_LINE_BYTES: usize = 4096;
pub const HIGHLIGHT_BUDGET: Duration = Duration::from_millis(500);

const PLAIN_TEXT: &str = "Plain Text";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxId(usize);

#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub bytes: usize,
    pub line_bytes: usize,
    pub budget: Duration,
}

impl Limits {
    pub(crate) const DEFAULT: Limits =
        Limits { bytes: MAX_HIGHLIGHT_BYTES, line_bytes: MAX_HIGHLIGHT_LINE_BYTES, budget: HIGHLIGHT_BUDGET };
}

fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

fn id_of(found: &SyntaxReference) -> Option<SyntaxId> {
    if found.name == PLAIN_TEXT {
        return None;
    }
    syntax_set().syntaxes().iter().position(|candidate| std::ptr::eq(candidate, found)).map(SyntaxId)
}

pub fn syntax_for(file_name: &str, first_line: &str) -> Option<SyntaxId> {
    let set = syntax_set();
    let extension = Path::new(file_name).extension().and_then(|extension| extension.to_str());
    let by_name = extension
        .and_then(|extension| set.find_syntax_by_extension(extension))
        .or_else(|| set.find_syntax_by_extension(file_name).filter(|_| !file_name.is_empty()));
    by_name.or_else(|| set.find_syntax_by_first_line(first_line)).and_then(id_of)
}

pub fn highlight(text: &str, syntax: SyntaxId) -> Highlighted {
    highlight_limited(text, syntax, Limits::DEFAULT)
}

fn plain_line(line: &str) -> Vec<Span> {
    if line.is_empty() {
        return Vec::new();
    }
    vec![Span { start: 0, end: line.len(), kind: TokenKind::Plain }]
}

fn push_span(spans: &mut Vec<Span>, start: usize, end: usize, kind: TokenKind) {
    if end <= start {
        return;
    }
    match spans.last_mut() {
        Some(last) if last.kind == kind && last.end == start => last.end = end,
        _ => spans.push(Span { start, end, kind }),
    }
}

fn coloured_line(line: &str, state: &mut ParseState, stack: &mut ScopeStack) -> Option<Vec<Span>> {
    let mut with_newline = String::with_capacity(line.len() + 1);
    with_newline.push_str(line);
    with_newline.push('\n');
    let operations = state.parse_line(&with_newline, syntax_set()).ok()?;
    let mut spans = Vec::new();
    let mut at = 0;
    for (offset, operation) in operations {
        let offset = offset.min(line.len());
        push_span(&mut spans, at, offset, kind_of_scopes(stack.as_slice()));
        at = at.max(offset);
        let _ = stack.apply(&operation);
    }
    push_span(&mut spans, at, line.len(), kind_of_scopes(stack.as_slice()));
    Some(spans)
}

pub(crate) fn highlight_limited(text: &str, syntax: SyntaxId, limits: Limits) -> Highlighted {
    let started = Instant::now();
    let set = syntax_set();
    let mut state = ParseState::new(&set.syntaxes()[syntax.0]);
    let mut stack = ScopeStack::new();
    let mut lines = Vec::new();
    for line in lines_of(text) {
        let line_start = line.as_ptr() as usize - text.as_ptr() as usize;
        let skip = line_start >= limits.bytes || line.len() > limits.line_bytes || started.elapsed() >= limits.budget;
        let spans = if skip { None } else { coloured_line(line, &mut state, &mut stack) };
        lines.push(spans.unwrap_or_else(|| plain_line(line)));
    }
    Highlighted { lines }
}

#[cfg(test)]
mod tests;

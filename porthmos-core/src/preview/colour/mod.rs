use porthmos_highlight::{Highlighted, Span, TokenKind, highlight, lines_of, syntax_for};
use porthmos_monitor::{LogLevel, detect_level};

const PLAIN_LOG_NAMES: [&str; 3] = ["syslog", "messages", "journal"];

pub fn looks_like_log(file_name: &str) -> bool {
    let name = file_name.to_ascii_lowercase();
    name.ends_with(".log") || name.contains(".log.") || PLAIN_LOG_NAMES.contains(&name.as_str())
}

fn kind_of_level(level: LogLevel) -> TokenKind {
    match level {
        LogLevel::Error => TokenKind::Error,
        LogLevel::Warn => TokenKind::Warn,
        LogLevel::Info | LogLevel::Debug | LogLevel::Unknown => TokenKind::Plain,
    }
}

fn level_colours(text: &str) -> Highlighted {
    let lines = lines_of(text)
        .into_iter()
        .map(|line| match line.is_empty() {
            true => Vec::new(),
            false => vec![Span { start: 0, end: line.len(), kind: kind_of_level(detect_level(line)) }],
        })
        .collect();
    Highlighted { lines }
}

pub fn colour_text(file_name: &str, text: &str) -> Option<Highlighted> {
    if looks_like_log(file_name) {
        return Some(level_colours(text));
    }
    let first_line = lines_of(text).first().copied().unwrap_or("");
    syntax_for(file_name, first_line).map(|syntax| highlight(text, syntax))
}

#[cfg(test)]
mod tests;

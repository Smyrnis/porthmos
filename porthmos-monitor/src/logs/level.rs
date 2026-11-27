use super::LogLevel;

const SCAN_CHARS: usize = 200;
const MAX_SYSLOG_PRIORITY: u8 = 191;

fn syslog_priority(text: &str) -> Option<LogLevel> {
    let digits = text.strip_prefix('<')?.split_once('>')?.0;
    if digits.is_empty() || digits.len() > 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    match digits.parse::<u8>().ok().filter(|value| *value <= MAX_SYSLOG_PRIORITY)? % 8 {
        0..=3 => Some(LogLevel::Error),
        4 => Some(LogLevel::Warn),
        5 | 6 => Some(LogLevel::Info),
        7 => Some(LogLevel::Debug),
        _ => None,
    }
}

fn word_level(word: &str) -> LogLevel {
    match word.to_ascii_lowercase().as_str() {
        "error" | "err" | "fatal" | "critical" | "crit" | "panic" | "emerg" | "failed" | "fail" => LogLevel::Error,
        "warn" | "warning" | "wrn" => LogLevel::Warn,
        "info" | "inf" | "notice" => LogLevel::Info,
        "debug" | "dbg" | "trace" => LogLevel::Debug,
        _ => LogLevel::Unknown,
    }
}

pub fn detect_level(text: &str) -> LogLevel {
    let end = text.char_indices().nth(SCAN_CHARS).map_or(text.len(), |(index, _)| index);
    let head = &text[..end];
    syslog_priority(head).unwrap_or_else(|| {
        head.split(|character: char| !character.is_alphanumeric()).map(word_level).min().unwrap_or(LogLevel::Unknown)
    })
}

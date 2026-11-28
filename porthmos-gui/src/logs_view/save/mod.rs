use std::path::PathBuf;

use chrono::NaiveDate;
use porthmos_core::edit::printable;

use super::{LogsState, TabName};

pub const MAX_SOURCE_BYTES: usize = 200;

pub fn save_text(state: &LogsState, skipped: impl Fn(u64) -> String) -> String {
    let capacity = state.view_lines(0..state.view_len()).iter().map(|stored| stored.line.text.len() + 1).sum();
    let mut text = String::with_capacity(capacity);
    for stored in state.view_lines(0..state.view_len()) {
        match stored.skipped {
            Some(count) => text.push_str(&skipped(count)),
            None => text.push_str(&stored.line.text),
        }
        text.push('\n');
    }
    text
}

pub fn default_file_name(state: &LogsState, date: NaiveDate) -> String {
    let source = match state.name() {
        TabName::Empty => String::new(),
        TabName::File(name) => name,
        TabName::Journal(Some(unit)) => unit,
        TabName::Journal(None) => "journal".to_string(),
    };
    let safe: String = source
        .chars()
        .map(
            |character| {
                if character.is_alphanumeric() || matches!(character, '.' | '_' | '-') { character } else { '_' }
            },
        )
        .collect();
    let source = cut(&safe, MAX_SOURCE_BYTES);
    let source = if source.is_empty() { "log".to_string() } else { source.to_string() };
    let source = if is_device_name(&source) { format!("_{source}") } else { source };
    format!("{source}-{}.log", date.format("%Y-%m-%d"))
}

fn cut(text: &str, limit: usize) -> &str {
    let mut end = limit.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn is_device_name(source: &str) -> bool {
    let stem = source.split('.').next().unwrap_or_default().to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

pub async fn write_lines(path: PathBuf, text: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || std::fs::write(&path, text).map_err(|error| printable(&error.to_string())))
        .await
        .map_err(|error| printable(&error.to_string()))?
}

#[cfg(test)]
mod tests;

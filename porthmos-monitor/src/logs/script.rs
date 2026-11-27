use super::{MAX_INITIAL_LINES, MAX_LOG_PATH_BYTES};
use crate::{UnitName, change_script};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogSource {
    File(String),
    Journal(Option<UnitName>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSourceError {
    EmptyPath,
    PathTooLong,
    BadCharacter,
}

fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn checked_path(path: &str) -> Result<&str, LogSourceError> {
    if path.is_empty() {
        Err(LogSourceError::EmptyPath)
    } else if path.len() > MAX_LOG_PATH_BYTES {
        Err(LogSourceError::PathTooLong)
    } else if path.contains(['\0', '\n']) {
        Err(LogSourceError::BadCharacter)
    } else {
        Ok(path)
    }
}

fn shell_path(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => format!("\"$HOME\"/{}", quoted(rest)),
        None => quoted(path),
    }
}

pub fn log_script(source: &LogSource, initial_lines: u32) -> Result<String, LogSourceError> {
    let lines = initial_lines.min(MAX_INITIAL_LINES);
    let command = match source {
        LogSource::File(path) => {
            format!("exec tail -n {lines} -F -- {} 2>&1 </dev/null", shell_path(checked_path(path)?))
        }
        LogSource::Journal(unit) => {
            let unit = unit.as_ref().map(|unit| format!(" -u {}", quoted(unit.as_str()))).unwrap_or_default();
            format!("exec journalctl -f -n {lines} --no-pager --all -o short-iso{unit} 2>&1 </dev/null")
        }
    };
    Ok(change_script(&command))
}

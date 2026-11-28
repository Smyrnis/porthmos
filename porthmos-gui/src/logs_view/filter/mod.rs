use porthmos_core::LogLevel;

use super::StoredLine;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogFilter {
    pub text: String,
    pub error: bool,
    pub warn: bool,
    pub info: bool,
    pub debug: bool,
}

impl LogFilter {
    pub fn is_active(&self) -> bool {
        !self.text.trim().is_empty() || self.error || self.warn || self.info || self.debug
    }

    pub fn toggle(&mut self, level: LogLevel) {
        match level {
            LogLevel::Error => self.error = !self.error,
            LogLevel::Warn => self.warn = !self.warn,
            LogLevel::Info => self.info = !self.info,
            LogLevel::Debug => self.debug = !self.debug,
            LogLevel::Unknown => {}
        }
    }

    pub fn level_on(&self, level: LogLevel) -> bool {
        match level {
            LogLevel::Error => self.error,
            LogLevel::Warn => self.warn,
            LogLevel::Info => self.info,
            LogLevel::Debug => self.debug,
            LogLevel::Unknown => false,
        }
    }

    pub fn passes(&self, stored: &StoredLine) -> bool {
        if !self.is_active() {
            return true;
        }
        stored.skipped.is_none() && self.level_passes(stored.line.level) && text_matches(&stored.line.text, &self.text)
    }

    fn level_passes(&self, level: LogLevel) -> bool {
        let any = self.error || self.warn || self.info || self.debug;
        !any || self.level_on(level)
    }
}

pub fn text_matches(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.is_ascii() {
        return haystack.as_bytes().windows(needle.len()).any(|window| window.eq_ignore_ascii_case(needle.as_bytes()));
    }
    fold(haystack).contains(&fold(needle))
}

fn fold(text: &str) -> String {
    text.to_lowercase().replace('ς', "σ")
}

#[cfg(test)]
mod tests;

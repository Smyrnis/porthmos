mod level;
mod script;
mod split;

pub use level::detect_level;
pub use script::{LogSource, LogSourceError, log_script};
pub use split::LineSplitter;

pub const MAX_LOG_LINE_BYTES: usize = 4096;
pub const MAX_LOG_PATH_BYTES: usize = 4096;
pub const MAX_INITIAL_LINES: u32 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub text: String,
    pub level: LogLevel,
}

#[cfg(test)]
mod tests;

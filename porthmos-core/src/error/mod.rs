use crate::i18n;
use std::{fmt::Display, time::Duration};

use porthmos_vfs::{ErrorKind, ProtocolError};

use crate::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn lifetime(self) -> Duration {
        match self {
            Severity::Info => Duration::from_secs(4),
            Severity::Warning => Duration::from_secs(8),
            Severity::Error => Duration::from_secs(15),
        }
    }
}

pub fn user_message(context: impl AsRef<str>, err: &dyn Display) -> String {
    Message::with_cause(context.as_ref(), err).to_string()
}

pub fn failure_report(context: impl AsRef<str>, err: &dyn Display) -> Message {
    Message::with_cause(context.as_ref(), err)
}

pub fn connection_cancelled() -> String {
    i18n::text("connection.cancelled")
}

pub fn search_incomplete_note(unreadable: usize) -> String {
    i18n::plural("search.incomplete", unreadable)
}

pub fn connect_failure_message(error: &ProtocolError, name: &str, protocol_display: &str) -> Message {
    match error.kind() {
        ErrorKind::Auth => failure_report(i18n::text_with("connection.auth_error", &[("name", name)]), error),
        ErrorKind::AuthRejected => Message::new(i18n::text_with("connection.auth_failed", &[("name", name)])),
        ErrorKind::SessionStart => failure_report(
            i18n::text_with("connection.session_start", &[("name", name), ("protocol", protocol_display)]),
            error,
        ),
        ErrorKind::Cancelled => Message::new(connection_cancelled()),
        _ => failure_report(i18n::text_with("connection.unable", &[("name", name)]), error),
    }
}

#[cfg(test)]
mod tests;

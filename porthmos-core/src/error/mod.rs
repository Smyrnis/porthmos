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

pub const CONNECTION_CANCELLED: &str = "Connection cancelled";

pub fn connect_failure_message(error: &ProtocolError, name: &str, protocol_display: &str) -> Message {
    match error.kind() {
        ErrorKind::Auth => failure_report(format!("Authentication error for {name}"), error),
        ErrorKind::AuthRejected => Message::new(format!("Authentication failed for {name}")),
        ErrorKind::SessionStart => {
            failure_report(format!("Connected to {name} but failed to start {protocol_display}"), error)
        }
        ErrorKind::Cancelled => Message::new(CONNECTION_CANCELLED),
        _ => failure_report(format!("Unable to connect to {name}"), error),
    }
}

#[cfg(test)]
mod tests;

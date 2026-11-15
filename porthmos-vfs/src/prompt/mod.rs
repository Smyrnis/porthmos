use std::fmt;

use async_trait::async_trait;

pub const PASSWORD_LABEL: &str = "Password";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    Password { username: String, name: String, secret: &'static str },
    TrustHostKey { name: String, host: String, port: u16, key_type: String, fingerprint: String },
    TrustCertificate { name: String, host: String, port: u16, fingerprint: String, subject: String, expires: String },
}

impl Question {
    pub fn is_risky_to_confirm(&self) -> bool {
        matches!(self, Question::TrustHostKey { .. } | Question::TrustCertificate { .. })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Answer {
    Password(String),
    Confirmed,
}

impl fmt::Debug for Answer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Answer::Password(_) => f.write_str("Password(<redacted>)"),
            Answer::Confirmed => f.write_str("Confirmed"),
        }
    }
}

#[async_trait]
pub trait Prompter: Send {
    async fn ask(&mut self, question: Question) -> Option<Answer>;
}

#[cfg(test)]
mod tests;

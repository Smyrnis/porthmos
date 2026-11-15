use std::fmt::{self, Display};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    summary: String,
    cause: Option<String>,
}

impl Message {
    pub fn new(summary: impl Into<String>) -> Self {
        Self { summary: summary.into(), cause: None }
    }

    pub fn with_cause(summary: impl Into<String>, cause: &dyn Display) -> Self {
        let cause = cause.to_string();
        Self { summary: summary.into(), cause: Some(cause).filter(|cause| !cause.is_empty()) }
    }

    pub fn parse(text: &str) -> Self {
        match text.split_once('\n') {
            Some((first, rest)) => Self {
                summary: first.strip_suffix(':').unwrap_or(first).to_string(),
                cause: Some(rest.to_string()).filter(|cause| !cause.is_empty()),
            },
            None => Self::new(text),
        }
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn cause(&self) -> Option<&str> {
        self.cause.as_deref()
    }

    pub fn one_line(&self) -> String {
        match &self.cause {
            Some(cause) => {
                let flattened: Vec<&str> = cause.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
                format!("{}: {}", self.summary, flattened.join(" "))
            }
            None => self.summary.clone(),
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(cause) => write!(f, "{}:\n{cause}", self.summary),
            None => f.write_str(&self.summary),
        }
    }
}

impl From<&str> for Message {
    fn from(text: &str) -> Self {
        Self::parse(text)
    }
}

impl From<String> for Message {
    fn from(text: String) -> Self {
        Self::parse(&text)
    }
}

impl Message {
    fn is_displayed_as(&self, text: &str) -> bool {
        match &self.cause {
            Some(cause) => text
                .strip_prefix(self.summary.as_str())
                .and_then(|rest| rest.strip_prefix(":\n"))
                .is_some_and(|rest| rest == cause),
            None => self.summary == text,
        }
    }
}

impl PartialEq<str> for Message {
    fn eq(&self, other: &str) -> bool {
        self.is_displayed_as(other)
    }
}

impl PartialEq<&str> for Message {
    fn eq(&self, other: &&str) -> bool {
        self.is_displayed_as(other)
    }
}

impl PartialEq<String> for Message {
    fn eq(&self, other: &String) -> bool {
        self.is_displayed_as(other)
    }
}

#[cfg(test)]
mod tests;

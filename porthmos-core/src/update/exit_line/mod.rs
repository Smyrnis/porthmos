use super::ApplyOutcome;
use crate::i18n;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitLine {
    Out(String),
    Err(String),
}

impl ApplyOutcome {
    pub fn exit_line(&self, program: &str) -> Option<ExitLine> {
        match self {
            ApplyOutcome::Installed { version } => {
                Some(ExitLine::Out(i18n::text_with("update.updated", &[("version", &version.to_string())])))
            }
            ApplyOutcome::Failed { version, message } => Some(ExitLine::Err(i18n::text_with(
                "update.exit_failed",
                &[("program", program), ("version", &version.to_string()), ("cause", message)],
            ))),
            ApplyOutcome::Nothing => None,
        }
    }
}

#[cfg(test)]
mod tests;

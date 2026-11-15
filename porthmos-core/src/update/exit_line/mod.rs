use super::ApplyOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitLine {
    Out(String),
    Err(String),
}

impl ApplyOutcome {
    pub fn exit_line(&self, program: &str) -> Option<ExitLine> {
        match self {
            ApplyOutcome::Installed { version } => Some(ExitLine::Out(format!("Porthmos updated to {version}."))),
            ApplyOutcome::Failed { version, message } => {
                Some(ExitLine::Err(format!("{program}: couldn't install the update to {version}: {message}")))
            }
            ApplyOutcome::Nothing => None,
        }
    }
}

#[cfg(test)]
mod tests;

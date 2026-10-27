use porthmos_core::{ApplyOutcome, HeadlessUpdate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Run,
    Update { check_only: bool },
    Print(String),
    Reject(String),
}

const USAGE: &str = "porthmos: a dual-panel file manager for local files and remote connections

Usage: porthmos [options]
       porthmos update [--check]

Commands:
  update          Download and install the latest version
  update --check  Report whether a newer version exists, install nothing

Options:
  -V, --version  Print the version and exit
  -h, --help     Print this help and exit";

pub fn interpret(args: impl IntoIterator<Item = String>) -> Invocation {
    let mut args = args.into_iter().peekable();
    if args.next_if(|first| first == "update").is_some() {
        return interpret_update(args);
    }
    for argument in args {
        match argument.as_str() {
            "--version" | "-V" => return Invocation::Print(format!("porthmos {}", env!("CARGO_PKG_VERSION"))),
            "--help" | "-h" => return Invocation::Print(USAGE.to_string()),
            option if option.len() > 1 && option.starts_with('-') => {
                return Invocation::Reject(format!("unknown option '{option}'"));
            }
            _ => {}
        }
    }
    Invocation::Run
}

fn interpret_update(args: impl Iterator<Item = String>) -> Invocation {
    let mut check_only = false;
    for argument in args {
        match argument.as_str() {
            "--version" | "-V" => return Invocation::Print(format!("porthmos {}", env!("CARGO_PKG_VERSION"))),
            "--help" | "-h" => return Invocation::Print(USAGE.to_string()),
            "--check" => check_only = true,
            option if option.len() > 1 && option.starts_with('-') => {
                return Invocation::Reject(format!("unknown option '{option}'"));
            }
            word => return Invocation::Reject(format!("unexpected argument '{word}'")),
        }
    }
    Invocation::Update { check_only }
}

pub fn describe_update(update: &HeadlessUpdate) -> String {
    match update {
        HeadlessUpdate::UpToDate { current } => format!("Porthmos {current} is up to date."),
        HeadlessUpdate::Available { version } => format!("Porthmos {version} is available."),
        HeadlessUpdate::Managed { version, reason } => format!(
            "Porthmos {version} is available. This copy is managed ({reason}); update it with your package manager."
        ),
        HeadlessUpdate::Installed { version } => format!("Porthmos updated to {version}."),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitLine {
    Out(String),
    Err(String),
}

pub fn install_message(outcome: &ApplyOutcome) -> Option<ExitLine> {
    match outcome {
        ApplyOutcome::Installed { version } => Some(ExitLine::Out(format!("Porthmos updated to {version}."))),
        ApplyOutcome::Failed { version, message } => {
            Some(ExitLine::Err(format!("porthmos: couldn't install the update to {version}: {message}")))
        }
        ApplyOutcome::Nothing => None,
    }
}

#[cfg(test)]
mod tests;

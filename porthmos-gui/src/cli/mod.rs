use porthmos_core::HeadlessUpdate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Run,
    Update { check_only: bool },
    Print(String),
    Reject(String),
}

const USAGE: &str = "porthmos-gui: a dual-panel file manager window for local files and remote connections

Usage: porthmos-gui [options]
       porthmos-gui update [--check]

Commands:
  update          Download and install the latest version
  update --check  Report whether a newer version exists, install nothing

Options:
  -V, --version  Print the version and exit
  -h, --help     Print this help and exit";

fn version_line() -> String {
    format!("porthmos-gui {}", env!("CARGO_PKG_VERSION"))
}

fn reject(argument: &str) -> Invocation {
    if argument.len() > 1 && argument.starts_with('-') {
        Invocation::Reject(format!("unknown option '{argument}'"))
    } else {
        Invocation::Reject(format!("unexpected argument '{argument}'"))
    }
}

pub fn interpret(args: impl IntoIterator<Item = String>) -> Invocation {
    let mut args = args.into_iter().peekable();
    if args.next_if(|first| first == "update").is_some() {
        return interpret_update(args);
    }
    match args.next().as_deref() {
        None => Invocation::Run,
        Some("--version" | "-V") => Invocation::Print(version_line()),
        Some("--help" | "-h") => Invocation::Print(USAGE.to_string()),
        Some(other) => reject(other),
    }
}

fn interpret_update(args: impl Iterator<Item = String>) -> Invocation {
    let mut check_only = false;
    for argument in args {
        match argument.as_str() {
            "--version" | "-V" => return Invocation::Print(version_line()),
            "--help" | "-h" => return Invocation::Print(USAGE.to_string()),
            "--check" => check_only = true,
            other => return reject(other),
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

#[cfg(test)]
mod tests;

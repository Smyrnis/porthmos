#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Run,
    Print(String),
    Reject(String),
}

const USAGE: &str = "porthmos: a dual-panel file manager for local files and remote connections

Usage: porthmos [options]

Options:
  -V, --version  Print the version and exit
  -h, --help     Print this help and exit";

pub fn interpret(args: impl IntoIterator<Item = String>) -> Invocation {
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

#[cfg(test)]
mod tests;

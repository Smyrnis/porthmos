use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use porthmos_core::edit::{Backslash, split_command_as};

use crate::editing::{CommandSpec, Platform, program_name};

const TERMINAL_TABLE: [&str; 8] =
    ["gnome-terminal", "ptyxis", "xfce4-terminal", "kitty", "alacritty", "foot", "wezterm", "xterm"];
const WRAPPER: &str = r#". "$1" || exit 1; rm -f -- "$1"; shift; exec "$@""#;
#[cfg(unix)]
const HANDOVER_DIRECTORY: &str = "handover";
const WINDOW_TITLE: &str = "Porthmos";
const COMMAND_PLACEHOLDER: &str = "{command}";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalChoice<'a> {
    pub setting: Option<&'a str>,
    pub variable: Option<&'a str>,
}

pub struct TerminalLaunch {
    pub spec: CommandSpec,
    pub env: Vec<(OsString, OsString)>,
    pub cleanup: Vec<PathBuf>,
    pub wait: bool,
}

impl fmt::Debug for TerminalLaunch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let env: Vec<(&OsString, &str)> = self.env.iter().map(|(key, _)| (key, "<redacted>")).collect();
        formatter
            .debug_struct("TerminalLaunch")
            .field("spec", &self.spec)
            .field("env", &env)
            .field("cleanup", &self.cleanup)
            .field("wait", &self.wait)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalError {
    NoneFound,
    Io(String),
}

impl fmt::Display for TerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalError::NoneFound => {
                write!(formatter, "No terminal program found. Set $TERMINAL to your terminal's command.")
            }
            TerminalError::Io(message) => write!(formatter, "Couldn't prepare the terminal command: {message}"),
        }
    }
}

impl std::error::Error for TerminalError {}

pub fn quote_for_shell(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn terminal_flags(name: &str, wait: bool) -> Vec<&'static str> {
    match name {
        "gnome-terminal" if wait => vec!["--wait", "--"],
        "gnome-terminal" => vec!["--"],
        "ptyxis" => vec!["-s", "--"],
        "xfce4-terminal" => vec!["--disable-server", "-x"],
        "kitty" | "foot" => Vec::new(),
        "wezterm" => vec!["start", "--always-new-process", "--"],
        _ => vec!["-e"],
    }
}

fn split_words(text: &str, platform: Platform) -> Option<Vec<String>> {
    let backslash = if platform == Platform::Windows { Backslash::Literal } else { Backslash::Escapes };
    split_command_as(text, backslash).ok().filter(|words| !words.is_empty())
}

fn attach(words: &[String], flag_name: &str, wait: bool, inner: &CommandSpec) -> CommandSpec {
    let program = OsString::from(&words[0]);
    let inner_words = || std::iter::once(inner.program.clone()).chain(inner.args.iter().cloned());
    let mut args: Vec<OsString> = Vec::new();
    if words.iter().any(|word| word == COMMAND_PLACEHOLDER) {
        for word in &words[1..] {
            if word == COMMAND_PLACEHOLDER {
                args.extend(inner_words());
            } else {
                args.push(OsString::from(word));
            }
        }
    } else {
        args.extend(words[1..].iter().map(OsString::from));
        args.extend(terminal_flags(flag_name, wait).into_iter().map(OsString::from));
        args.extend(inner_words());
    }
    CommandSpec { program, args }
}

fn candidates(choice: &TerminalChoice<'_>, platform: Platform) -> Vec<Vec<String>> {
    let mut list: Vec<Vec<String>> = [choice.setting, choice.variable]
        .into_iter()
        .flatten()
        .filter_map(|text| split_words(text, platform))
        .collect();
    list.push(vec!["x-terminal-emulator".to_string()]);
    list.extend(TERMINAL_TABLE.iter().map(|name| vec![name.to_string()]));
    list
}

fn on_linux(
    inner: &CommandSpec, wait: bool, lookup: &dyn Fn(&str) -> Option<String>, choice: &TerminalChoice<'_>,
) -> Result<CommandSpec, TerminalError> {
    for words in candidates(choice, Platform::Linux) {
        let Some(resolved) = lookup(&words[0]) else {
            continue;
        };
        return Ok(attach(&words, &program_name(&resolved), wait, inner));
    }
    Err(TerminalError::NoneFound)
}

fn chosen_by_setting(
    inner: &CommandSpec, wait: bool, platform: Platform, choice: &TerminalChoice<'_>,
) -> Option<CommandSpec> {
    let words = split_words(choice.setting?, platform)?;
    Some(attach(&words, &program_name(&words[0]), wait, inner))
}

fn applescript_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn on_macos(inner: &CommandSpec, wait: bool) -> CommandSpec {
    let words: Vec<String> = std::iter::once(&inner.program)
        .chain(inner.args.iter())
        .map(|word| quote_for_shell(&word.to_string_lossy()))
        .collect();
    let command = applescript_text(&format!("{}; exit", words.join(" ")));
    let mut lines = vec![
        "tell application \"Terminal\"".to_string(),
        "activate".to_string(),
        format!("set porthmosTab to do script \"{command}\""),
    ];
    if wait {
        lines.extend(
            ["try", "repeat while busy of porthmosTab", "delay 0.5", "end repeat", "end try"].map(str::to_string),
        );
    }
    lines.push("end tell".to_string());
    let args = lines.into_iter().flat_map(|line| [OsString::from("-e"), OsString::from(line)]).collect();
    CommandSpec { program: OsString::from("osascript"), args }
}

fn on_windows(inner: CommandSpec, wait: bool) -> CommandSpec {
    let mut args: Vec<OsString> = ["/c", "start", WINDOW_TITLE].map(OsString::from).into();
    if wait {
        args.push(OsString::from("/wait"));
    }
    args.push(inner.program);
    args.extend(inner.args);
    CommandSpec { program: OsString::from("cmd.exe"), args }
}

#[cfg(unix)]
fn io_error(error: std::io::Error) -> TerminalError {
    TerminalError::Io(error.to_string())
}

fn valid_variable_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn env_script(env: &[(OsString, OsString)]) -> Result<String, TerminalError> {
    let mut script = String::new();
    for (key, value) in env {
        let name = key.to_str().filter(|name| valid_variable_name(name)).ok_or_else(|| {
            TerminalError::Io(format!("{} is not a valid environment variable name", key.to_string_lossy()))
        })?;
        let text = value.to_str().ok_or_else(|| TerminalError::Io(format!("the value of {name} is not valid text")))?;
        script.push_str(&format!("export {name}={}\n", quote_for_shell(text)));
    }
    Ok(script)
}

#[cfg(unix)]
fn write_handover(script: &str, state_dir: &Path) -> Result<PathBuf, TerminalError> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let directory = state_dir.join(HANDOVER_DIRECTORY);
    match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.is_dir() => {
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).map_err(io_error)?
        }
        Ok(_) => return Err(TerminalError::Io(format!("{} is not a directory", directory.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&directory).map_err(io_error)?
        }
        Err(error) => return Err(io_error(error)),
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let name = format!("env-{}-{nanos}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
    let path = directory.join(name);
    let mut file =
        std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(io_error)?;
    if let Err(error) = file.write_all(script.as_bytes()) {
        let _ = std::fs::remove_file(&path);
        return Err(io_error(error));
    }
    Ok(path)
}

#[cfg(not(unix))]
fn write_handover(_script: &str, _state_dir: &Path) -> Result<PathBuf, TerminalError> {
    Err(TerminalError::Io("handing over the environment through a file needs a Unix system".to_string()))
}

pub fn hand_over(
    inner: CommandSpec, env: &[(OsString, OsString)], state_dir: &Path,
) -> Result<(CommandSpec, Vec<PathBuf>), TerminalError> {
    let script = env_script(env)?;
    let file = write_handover(&script, state_dir)?;
    let mut args: Vec<OsString> = vec![OsString::from("-c"), OsString::from(WRAPPER), OsString::from("sh")];
    args.push(file.clone().into_os_string());
    args.push(inner.program);
    args.extend(inner.args);
    Ok((CommandSpec { program: OsString::from("sh"), args }, vec![file]))
}

fn unix_inner(
    inner: CommandSpec, env: &[(OsString, OsString)], state_dir: &Path,
) -> Result<(CommandSpec, Vec<PathBuf>), TerminalError> {
    if env.is_empty() { Ok((inner, Vec::new())) } else { hand_over(inner, env, state_dir) }
}

pub fn launch(
    inner: CommandSpec, inner_env: Vec<(OsString, OsString)>, wait: bool, platform: Platform,
    lookup: &dyn Fn(&str) -> Option<String>, choice: &TerminalChoice<'_>, state_dir: &Path,
) -> Result<TerminalLaunch, TerminalError> {
    match platform {
        Platform::Windows => {
            let spec = chosen_by_setting(&inner, wait, platform, choice).unwrap_or_else(|| on_windows(inner, wait));
            Ok(TerminalLaunch { spec, env: inner_env, cleanup: Vec::new(), wait })
        }
        Platform::MacOs => {
            let (inner, cleanup) = unix_inner(inner, &inner_env, state_dir)?;
            let spec = chosen_by_setting(&inner, wait, platform, choice).unwrap_or_else(|| on_macos(&inner, wait));
            Ok(TerminalLaunch { spec, env: Vec::new(), cleanup, wait })
        }
        Platform::Linux => {
            let (inner, cleanup) = unix_inner(inner, &inner_env, state_dir)?;
            match on_linux(&inner, wait, lookup, choice) {
                Ok(spec) => Ok(TerminalLaunch { spec, env: Vec::new(), cleanup, wait }),
                Err(error) => {
                    for file in &cleanup {
                        let _ = std::fs::remove_file(file);
                    }
                    Err(error)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

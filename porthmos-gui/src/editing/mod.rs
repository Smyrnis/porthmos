use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitStatus};

use porthmos_core::edit::{EditorCommand, EditorExit, printable};

const TERMINAL_EDITORS: [&str; 14] =
    ["vi", "vim", "nvim", "view", "nano", "pico", "micro", "ed", "ne", "joe", "jed", "kak", "hx", "helix"];
const EMACS_TERMINAL_FLAGS: [&str; 3] = ["-nw", "-t", "--no-window-system"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
}

impl CommandSpec {
    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKind {
    Window,
    Terminal,
    DefaultApp,
}

pub fn program_name(program: &str) -> String {
    let file = program.rsplit(['/', '\\']).next().unwrap_or(program).to_lowercase();
    file.strip_suffix(".exe").map(str::to_string).unwrap_or(file)
}

pub fn classify(editor: &EditorCommand, platform: Platform) -> EditorKind {
    let name = program_name(&editor.program);
    if name == "emacs" {
        let terminal = editor.args.iter().any(|arg| EMACS_TERMINAL_FLAGS.contains(&arg.as_str()));
        return if terminal { EditorKind::Terminal } else { EditorKind::Window };
    }
    if !TERMINAL_EDITORS.contains(&name.as_str()) {
        return EditorKind::Window;
    }
    let bare_vi = editor.program == "vi" && editor.args.is_empty();
    if bare_vi && platform != Platform::Windows {
        return EditorKind::DefaultApp;
    }
    EditorKind::Terminal
}

pub fn editor_spec(editor: &EditorCommand, file: &Path) -> CommandSpec {
    let mut args: Vec<OsString> = editor.args.iter().map(OsString::from).collect();
    args.push(file.as_os_str().to_owned());
    CommandSpec { program: OsString::from(&editor.program), args }
}

pub fn default_app_spec(file: &Path, platform: Platform) -> CommandSpec {
    let file = file.as_os_str().to_owned();
    let (program, mut args): (&str, Vec<OsString>) = match platform {
        Platform::Linux => ("xdg-open", Vec::new()),
        Platform::MacOs => ("open", vec![OsString::from("-t")]),
        Platform::Windows => ("cmd", vec![OsString::from("/c"), OsString::from("start"), OsString::new()]),
    };
    args.push(file);
    CommandSpec { program: OsString::from(program), args }
}

pub fn editor_exit(result: std::io::Result<ExitStatus>) -> EditorExit {
    match result {
        Ok(status) if status.success() => EditorExit::Success,
        Ok(status) => EditorExit::Status(status.code().unwrap_or(-1)),
        Err(error) => EditorExit::LaunchFailed(error.to_string()),
    }
}

pub fn waiting_notice(name: &str) -> String {
    format!("Editing {}. Save the file in your editor, then press Done.", printable(name))
}

#[cfg(test)]
mod tests;

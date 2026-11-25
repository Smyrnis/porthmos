use std::ffi::OsString;
use std::path::Path;

use super::{CommandSpec, EditorKind, Platform, classify, default_app_spec, editor_exit, editor_spec, url_spec};
use crate::edit::{EditorCommand, EditorExit};

fn editor(program: &str, args: &[&str]) -> EditorCommand {
    EditorCommand { program: program.to_string(), args: args.iter().map(|arg| arg.to_string()).collect() }
}

fn strings(values: &[OsString]) -> Vec<String> {
    values.iter().map(|value| value.to_string_lossy().to_string()).collect()
}

#[test]
fn classify_known_terminal_editors() {
    for name in ["vim", "nvim", "view", "nano", "pico", "micro", "ed", "ne", "joe", "jed", "kak", "hx", "helix"] {
        for program in [name.to_string(), format!("/usr/bin/{name}"), format!("{name}.exe"), name.to_uppercase()] {
            assert_eq!(classify(&editor(&program, &[]), Platform::Linux), EditorKind::Terminal, "{program}");
        }
    }
}

#[test]
fn classify_a_window_editor() {
    for program in ["code", "gedit", "notepad.exe", "subl", "/opt/zed/bin/zed", "C:\\Tools\\kate.exe"] {
        assert_eq!(classify(&editor(program, &["--wait"]), Platform::Linux), EditorKind::Window, "{program}");
    }
}

#[test]
fn windows_style_paths_are_split_on_either_separator() {
    assert_eq!(classify(&editor("C:\\tools\\nvim.exe", &[]), Platform::Windows), EditorKind::Terminal);
}

#[test]
fn a_bare_vi_is_the_default_app_on_unix_but_not_on_windows() {
    assert_eq!(classify(&editor("vi", &[]), Platform::Linux), EditorKind::DefaultApp);
    assert_eq!(classify(&editor("vi", &[]), Platform::MacOs), EditorKind::DefaultApp);
    assert_eq!(classify(&editor("vi", &[]), Platform::Windows), EditorKind::Terminal);
}

#[test]
fn vi_with_arguments_is_a_terminal_editor() {
    assert_eq!(classify(&editor("vi", &["-u", "NONE"]), Platform::Linux), EditorKind::Terminal);
    assert_eq!(classify(&editor("/usr/bin/vi", &[]), Platform::Linux), EditorKind::Terminal);
}

#[test]
fn emacs_is_a_terminal_editor_only_with_nw() {
    assert_eq!(classify(&editor("emacs", &[]), Platform::Linux), EditorKind::Window);
    assert_eq!(classify(&editor("emacs", &["-nw"]), Platform::Linux), EditorKind::Terminal);
    assert_eq!(classify(&editor("emacs", &["-q", "-t"]), Platform::Linux), EditorKind::Terminal);
    assert_eq!(classify(&editor("emacs", &["--no-window-system"]), Platform::Linux), EditorKind::Terminal);
}

#[test]
fn editor_spec_appends_the_file_after_the_arguments() {
    let spec = editor_spec(&editor("code", &["--wait", "--new-window"]), Path::new("/tmp/a b.txt"));

    assert_eq!(spec.program, OsString::from("code"));
    assert_eq!(strings(&spec.args), ["--wait", "--new-window", "/tmp/a b.txt"]);
}

#[test]
fn default_app_specs_per_platform() {
    let file = Path::new("/tmp/a.txt");

    let linux = default_app_spec(file, Platform::Linux);
    assert_eq!((linux.program, strings(&linux.args)), (OsString::from("xdg-open"), vec!["/tmp/a.txt".to_string()]));
    let mac = default_app_spec(file, Platform::MacOs);
    assert_eq!(
        (mac.program, strings(&mac.args)),
        (OsString::from("open"), vec!["-t".to_string(), "/tmp/a.txt".to_string()])
    );
    let windows = default_app_spec(file, Platform::Windows);
    assert_eq!(windows.program, OsString::from("cmd"));
    assert_eq!(strings(&windows.args), ["/c", "start", "", "/tmp/a.txt"]);
}

#[test]
fn to_command_carries_the_program_and_arguments() {
    let spec =
        CommandSpec { program: OsString::from("echo"), args: vec![OsString::from("one"), OsString::from("two")] };

    let command = spec.to_command();

    assert_eq!(command.get_program(), "echo");
    assert_eq!(command.get_args().map(|arg| arg.to_string_lossy().to_string()).collect::<Vec<_>>(), ["one", "two"]);
}

#[test]
fn the_current_platform_is_one_of_the_three() {
    let current = Platform::current();

    assert!(matches!(current, Platform::Linux | Platform::MacOs | Platform::Windows));
}

#[test]
fn the_file_is_the_last_argument_and_nothing_goes_through_a_shell() {
    let command =
        editor_spec(&editor("code", &["--wait", "-n"]), Path::new("/tmp/-weird name; rm -rf.txt")).to_command();

    assert_eq!(command.get_program(), "code");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["--wait", "-n", "/tmp/-weird name; rm -rf.txt"]);
}

#[cfg(unix)]
const SHELL: &str = "sh";
#[cfg(windows)]
const SHELL: &str = "cmd";
#[cfg(unix)]
const SHELL_FLAG: &str = "-c";
#[cfg(windows)]
const SHELL_FLAG: &str = "/c";

#[test]
fn the_exit_status_maps_to_the_editor_outcome() {
    let ok = std::process::Command::new(SHELL).args([SHELL_FLAG, "exit 0"]).status();
    let failed = std::process::Command::new(SHELL).args([SHELL_FLAG, "exit 3"]).status();
    let missing = std::process::Command::new("/nonexistent/editor").status();

    assert_eq!(editor_exit(ok), EditorExit::Success);
    assert_eq!(editor_exit(failed), EditorExit::Status(3));
    assert!(matches!(editor_exit(missing), EditorExit::LaunchFailed(message) if !message.is_empty()));
}

#[cfg(unix)]
#[test]
fn a_process_killed_by_a_signal_is_a_failed_status() {
    let killed = std::process::Command::new("sh").args(["-c", "kill -9 $$"]).status();

    assert_eq!(editor_exit(killed), EditorExit::Status(-1));
}

#[test]
fn a_launch_error_keeps_its_message() {
    let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such program");

    assert_eq!(editor_exit(Err(error)), EditorExit::LaunchFailed("no such program".to_string()));
}

#[test]
fn url_specs_per_platform_never_go_through_a_shell() {
    let url = "https://example.org/?a=1&b=2";

    let linux = url_spec(url, Platform::Linux);
    assert_eq!((linux.program, strings(&linux.args)), (OsString::from("xdg-open"), vec![url.to_string()]));
    let mac = url_spec(url, Platform::MacOs);
    assert_eq!((mac.program, strings(&mac.args)), (OsString::from("open"), vec![url.to_string()]));
    let windows = url_spec(url, Platform::Windows);
    assert_eq!(windows.program, OsString::from("rundll32"));
    assert_eq!(strings(&windows.args), ["url.dll,FileProtocolHandler", url]);
}

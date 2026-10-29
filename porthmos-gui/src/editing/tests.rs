use std::ffi::OsString;
use std::path::Path;

use porthmos_core::edit::EditorCommand;

use super::{CommandSpec, EditorKind, Platform, classify, default_app_spec, editor_spec, waiting_notice};

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

#[cfg(unix)]
#[test]
fn editor_exit_maps_success_status_and_launch_failure() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use porthmos_core::edit::EditorExit;

    use super::editor_exit;

    assert_eq!(editor_exit(Ok(ExitStatus::from_raw(0))), EditorExit::Success);
    assert_eq!(editor_exit(Ok(ExitStatus::from_raw(3 << 8))), EditorExit::Status(3));
    assert_eq!(editor_exit(Ok(ExitStatus::from_raw(9))), EditorExit::Status(-1));
    let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no such program");
    assert_eq!(editor_exit(Err(error)), EditorExit::LaunchFailed("no such program".to_string()));
}

#[test]
fn waiting_notice_makes_names_printable() {
    assert_eq!(waiting_notice("a.txt"), "Editing a.txt. Save the file in your editor, then press Done.");
    let hostile = waiting_notice("evil\u{1b}[31m\nname");
    assert!(!hostile.contains('\u{1b}'));
    assert!(!hostile.contains('\n'));
}

#[test]
fn the_current_platform_is_one_of_the_three() {
    let current = Platform::current();

    assert!(matches!(current, Platform::Linux | Platform::MacOs | Platform::Windows));
}

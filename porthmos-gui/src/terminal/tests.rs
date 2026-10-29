use crate::editing::{CommandSpec, Platform};
use std::ffi::OsString;

use super::{TerminalError, TerminalLaunch, launch, quote_for_shell};

fn inner() -> CommandSpec {
    CommandSpec {
        program: OsString::from("ssh"),
        args: vec![OsString::from("-p"), OsString::from("22"), OsString::from("host")],
    }
}

fn secret_env() -> Vec<(OsString, OsString)> {
    vec![(OsString::from("SSHPASS"), OsString::from("s3cr3t-value"))]
}

fn strings(values: &[OsString]) -> Vec<String> {
    values.iter().map(|value| value.to_string_lossy().to_string()).collect()
}

fn only(names: &'static [&'static str]) -> impl Fn(&str) -> Option<String> {
    move |program| names.contains(&program).then(|| program.to_string())
}

fn run(platform: Platform, wait: bool, names: &'static [&'static str], variable: Option<&str>) -> TerminalLaunch {
    let dir = tempfile::tempdir().unwrap();
    launch(inner(), Vec::new(), wait, platform, &only(names), variable, dir.path()).unwrap()
}

fn argv(launch: &TerminalLaunch) -> Vec<String> {
    strings(&launch.spec.args)
}

const INNER_ARGS: [&str; 3] = ["-p", "22", "host"];

fn expected(flags: &[&str]) -> Vec<String> {
    flags.iter().chain(["ssh"].iter()).chain(INNER_ARGS.iter()).map(|part| part.to_string()).collect()
}

#[test]
fn each_verified_terminal_gets_its_flags() {
    let cases: [(&'static str, &[&str], &[&str]); 8] = [
        ("gnome-terminal", &["--wait", "--"], &["--"]),
        ("ptyxis", &["-s", "--"], &["-s", "--"]),
        ("xfce4-terminal", &["--disable-server", "-x"], &["--disable-server", "-x"]),
        ("kitty", &[], &[]),
        ("alacritty", &["-e"], &["-e"]),
        ("foot", &[], &[]),
        ("wezterm", &["start", "--always-new-process", "--"], &["start", "--always-new-process", "--"]),
        ("xterm", &["-e"], &["-e"]),
    ];
    for (name, waiting, detached) in cases {
        let names: &'static [&'static str] = Box::leak(vec![name].into_boxed_slice());
        let waited = run(Platform::Linux, true, names, None);
        assert_eq!(waited.spec.program, OsString::from(name));
        assert_eq!(argv(&waited), expected(waiting), "{name} waiting");
        assert!(waited.wait);
        let free = run(Platform::Linux, false, names, None);
        assert_eq!(argv(&free), expected(detached), "{name} detached");
        assert!(!free.wait);
    }
}

#[test]
fn terminal_variable_wins_over_the_table() {
    let launch = run(Platform::Linux, false, &["xterm", "foot"], Some("foot"));

    assert_eq!(launch.spec.program, OsString::from("foot"));
}

#[test]
fn terminal_variable_may_carry_arguments_and_a_known_name_picks_its_flags() {
    let launch = run(Platform::Linux, false, &["alacritty"], Some("alacritty --title Porthmos"));

    assert_eq!(launch.spec.program, OsString::from("alacritty"));
    assert_eq!(argv(&launch)[..3], ["--title", "Porthmos", "-e"]);
}

#[test]
fn an_unknown_terminal_variable_gets_dash_e() {
    let launch = run(Platform::Linux, false, &["mystery-term"], Some("mystery-term"));

    assert_eq!(argv(&launch), expected(&["-e"]));
}

#[test]
fn a_terminal_variable_that_is_not_installed_falls_through() {
    let launch = run(Platform::Linux, false, &["xterm"], Some("missing-term"));

    assert_eq!(launch.spec.program, OsString::from("xterm"));
}

#[test]
fn x_terminal_emulator_comes_before_the_table() {
    let dir = tempfile::tempdir().unwrap();
    let lookup = |program: &str| match program {
        "x-terminal-emulator" => Some("ptyxis".to_string()),
        "xterm" => Some("xterm".to_string()),
        _ => None,
    };

    let launch = launch(inner(), Vec::new(), true, Platform::Linux, &lookup, None, dir.path()).unwrap();

    assert_eq!(launch.spec.program, OsString::from("x-terminal-emulator"));
    assert_eq!(argv(&launch), expected(&["-s", "--"]));
}

#[test]
fn no_terminal_found_is_an_error_with_the_hint() {
    let dir = tempfile::tempdir().unwrap();

    let error = launch(inner(), Vec::new(), true, Platform::Linux, &only(&[]), None, dir.path()).unwrap_err();

    assert_eq!(error, TerminalError::NoneFound);
    assert_eq!(error.to_string(), "No terminal program found. Set $TERMINAL to your terminal's command.");
}

#[test]
fn windows_wraps_in_cmd_start_with_and_without_wait() {
    let waited = run(Platform::Windows, true, &[], None);
    assert_eq!(waited.spec.program, OsString::from("cmd.exe"));
    assert_eq!(argv(&waited), ["/c", "start", "Porthmos", "/wait", "ssh", "-p", "22", "host"]);

    let free = run(Platform::Windows, false, &[], None);
    assert_eq!(argv(&free), ["/c", "start", "Porthmos", "ssh", "-p", "22", "host"]);
}

#[test]
fn windows_hands_the_environment_to_the_launcher_process() {
    let dir = tempfile::tempdir().unwrap();

    let launched = launch(inner(), secret_env(), true, Platform::Windows, &only(&[]), None, dir.path()).unwrap();

    assert_eq!(launched.env, secret_env());
    assert!(launched.cleanup.is_empty());
    assert!(!strings(&launched.spec.args).iter().any(|arg| arg.contains("s3cr3t")));
}

#[test]
fn macos_wraps_in_osascript_and_quotes_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let tricky = CommandSpec { program: OsString::from("vim"), args: vec![OsString::from("it's a \"file\".txt")] };

    let launched = launch(tricky, Vec::new(), false, Platform::MacOs, &only(&[]), None, dir.path()).unwrap();

    assert_eq!(launched.spec.program, OsString::from("osascript"));
    let script = argv(&launched).join("\n");
    assert!(script.contains("tell application \"Terminal\""), "{script}");
    assert!(script.contains("do script"), "{script}");
    assert!(script.contains("'vim' 'it'\\\\''s a \\\"file\\\".txt'; exit"), "{script}");
    assert!(!script.contains("busy"), "{script}");
}

#[test]
fn macos_waits_on_the_busy_flag_when_asked() {
    let dir = tempfile::tempdir().unwrap();

    let launched = launch(inner(), Vec::new(), true, Platform::MacOs, &only(&[]), None, dir.path()).unwrap();

    assert!(argv(&launched).join("\n").contains("busy of"));
}

#[test]
fn quote_for_shell_handles_quotes_spaces_and_newlines() {
    assert_eq!(quote_for_shell("plain"), "'plain'");
    assert_eq!(quote_for_shell(""), "''");
    assert_eq!(quote_for_shell("a b"), "'a b'");
    assert_eq!(quote_for_shell("it's"), "'it'\\''s'");
    assert_eq!(quote_for_shell("a\nb"), "'a\nb'");
    assert_eq!(quote_for_shell("$(x) `y` \"z\""), "'$(x) `y` \"z\"'");
}

#[test]
fn env_values_never_appear_in_the_argv() {
    for platform in [Platform::Linux, Platform::MacOs, Platform::Windows] {
        let dir = tempfile::tempdir().unwrap();

        let launched = launch(inner(), secret_env(), true, platform, &only(&["xterm"]), None, dir.path()).unwrap();

        let all = format!("{} {}", launched.spec.program.to_string_lossy(), argv(&launched).join(" "));
        assert!(!all.contains("s3cr3t-value"), "{platform:?}: {all}");
    }
}

#[test]
fn the_debug_text_redacts_env_values() {
    let dir = tempfile::tempdir().unwrap();
    let launched = launch(inner(), secret_env(), true, Platform::Windows, &only(&[]), None, dir.path()).unwrap();

    let text = format!("{launched:?}");

    assert!(text.contains("SSHPASS"));
    assert!(text.contains("<redacted>"));
    assert!(!text.contains("s3cr3t-value"));
}

#[test]
fn no_env_means_no_file_and_no_wrapper() {
    let dir = tempfile::tempdir().unwrap();

    let launched = launch(inner(), Vec::new(), true, Platform::Linux, &only(&["xterm"]), None, dir.path()).unwrap();

    assert!(launched.cleanup.is_empty());
    assert!(launched.env.is_empty());
    assert_eq!(argv(&launched), expected(&["-e"]));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[cfg(unix)]
mod unix_handover {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::super::hand_over;

    use super::*;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_env_file_is_mode_0600_in_a_0700_directory() {
        let dir = tempfile::tempdir().unwrap();

        let (_, cleanup) = hand_over(inner(), &secret_env(), dir.path()).unwrap();

        let file = &cleanup[0];
        assert_eq!(mode(file), 0o600);
        assert_eq!(mode(file.parent().unwrap()), 0o700);
        assert!(file.starts_with(dir.path()));
    }

    #[test]
    fn an_existing_loose_directory_is_tightened() {
        let dir = tempfile::tempdir().unwrap();
        let handover = dir.path().join("handover");
        std::fs::create_dir(&handover).unwrap();
        std::fs::set_permissions(&handover, std::fs::Permissions::from_mode(0o755)).unwrap();

        hand_over(inner(), &secret_env(), dir.path()).unwrap();

        assert_eq!(mode(&handover), 0o700);
    }

    #[test]
    fn a_symlinked_handover_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), dir.path().join("handover")).unwrap();

        let error = hand_over(inner(), &secret_env(), dir.path()).unwrap_err();

        assert!(matches!(error, TerminalError::Io(_)));
        assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0);
    }

    #[test]
    fn cleanup_lists_the_env_file() {
        let dir = tempfile::tempdir().unwrap();

        let launched =
            launch(inner(), secret_env(), true, Platform::Linux, &only(&["xterm"]), None, dir.path()).unwrap();

        assert_eq!(launched.cleanup.len(), 1);
        assert!(launched.cleanup[0].exists());
        assert!(launched.env.is_empty());
    }

    #[test]
    fn the_wrapper_sources_then_deletes_the_file_and_runs_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let program = CommandSpec { program: OsString::from("printenv"), args: vec![OsString::from("TOKEN")] };
        let value = "it's \"quoted\" $(touch pwned) `x` \\ and\nnewline";
        let env = vec![(OsString::from("TOKEN"), OsString::from(value))];

        let (wrapped, cleanup) = hand_over(program, &env, dir.path()).unwrap();
        let output = wrapped.to_command().output().unwrap();

        assert_eq!(String::from_utf8_lossy(&output.stdout), format!("{value}\n"));
        assert!(!cleanup[0].exists());
        assert!(!dir.path().join("pwned").exists());
        assert!(!Path::new("pwned").exists());
    }

    #[test]
    fn the_wrapper_keeps_program_arguments_intact() {
        let dir = tempfile::tempdir().unwrap();
        let program = CommandSpec {
            program: OsString::from("printf"),
            args: vec![OsString::from("%s|%s"), OsString::from("a b"), OsString::from("$HOME")],
        };

        let (wrapped, _) = hand_over(program, &secret_env(), dir.path()).unwrap();
        let output = wrapped.to_command().output().unwrap();

        assert_eq!(String::from_utf8_lossy(&output.stdout), "a b|$HOME");
    }

    #[test]
    fn the_wrapper_argv_holds_the_path_and_never_the_value() {
        let dir = tempfile::tempdir().unwrap();

        let (wrapped, _) = hand_over(inner(), &secret_env(), dir.path()).unwrap();

        assert_eq!(wrapped.program, OsString::from("sh"));
        assert!(!strings(&wrapped.args).iter().any(|arg| arg.contains("s3cr3t")));
    }

    #[test]
    fn a_failed_state_dir_gives_an_io_error_without_the_value() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("not-a-directory");
        std::fs::write(&blocker, b"x").unwrap();

        let error = hand_over(inner(), &secret_env(), &blocker).unwrap_err();

        assert!(matches!(error, TerminalError::Io(_)));
        assert!(!error.to_string().contains("s3cr3t-value"));
        assert!(error.to_string().starts_with("Couldn't prepare the terminal command: "));
    }

    #[test]
    fn a_bad_variable_name_is_refused_naming_only_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let env = vec![(OsString::from("BAD NAME; rm -rf"), OsString::from("s3cr3t-value"))];

        let error = hand_over(inner(), &env, dir.path()).unwrap_err();

        assert!(matches!(error, TerminalError::Io(_)));
        assert!(!error.to_string().contains("s3cr3t-value"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn no_terminal_found_leaves_no_env_file_behind() {
        let dir = tempfile::tempdir().unwrap();

        let error = launch(inner(), secret_env(), true, Platform::Linux, &only(&[]), None, dir.path()).unwrap_err();

        assert_eq!(error, TerminalError::NoneFound);
        let leftovers = std::fs::read_dir(dir.path().join("handover")).unwrap().count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn a_failed_launch_can_delete_what_it_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let launched =
            launch(inner(), secret_env(), true, Platform::Linux, &only(&["xterm"]), None, dir.path()).unwrap();

        for file in &launched.cleanup {
            std::fs::remove_file(file).unwrap();
        }

        assert!(!launched.cleanup[0].exists());
    }
}

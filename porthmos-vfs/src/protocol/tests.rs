use super::*;

#[test]
fn target_debug_redacts_the_password() {
    let target = Target {
        name: "web".into(),
        host: "example.com".into(),
        port: 22,
        username: "deploy".into(),
        password: Some("hunter2".into()),
        options: BTreeMap::new(),
    };
    let printed = format!("{target:?}");
    assert!(!printed.contains("hunter2"));
    assert!(printed.contains("<redacted>"));
}

#[test]
fn shell_invocation_debug_redacts_environment_values() {
    let invocation = ShellInvocation {
        program: "sshpass".into(),
        args: vec!["-e".into()],
        env: vec![("SSHPASS".into(), "hunter2".into())],
    };
    let printed = format!("{invocation:?}");
    assert!(!printed.contains("hunter2"));
    assert!(printed.contains("SSHPASS"));
}

#[test]
fn target_option_reads_protocol_specific_keys() {
    let mut options = BTreeMap::new();
    options.insert("identity_file".to_string(), "/k".to_string());
    let target = Target { name: "n".into(), host: "h".into(), port: 22, username: "u".into(), password: None, options };
    assert_eq!(target.option("identity_file"), Some("/k"));
    assert_eq!(target.option("remote_path"), None);
}

#[test]
fn shell_invocation_builds_a_command_with_its_program_args_and_env() {
    let invocation = ShellInvocation {
        program: "ssh".into(),
        args: vec!["-p".into(), "22".into()],
        env: vec![("SSHPASS".into(), "x".into())],
    };
    let command = invocation.to_command();
    assert_eq!(command.get_program(), "ssh");
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["-p", "22"]);
    assert_eq!(
        command.get_envs().collect::<Vec<_>>(),
        [(std::ffi::OsStr::new("SSHPASS"), Some(std::ffi::OsStr::new("x")))]
    );
}

struct Minimal;

#[async_trait]
impl Protocol for Minimal {
    fn id(&self) -> &'static str {
        "minimal"
    }

    fn display_name(&self) -> &'static str {
        "Minimal"
    }

    fn default_port(&self) -> u16 {
        4242
    }

    async fn connect(
        &self, _target: &Target, _prompter: &mut dyn Prompter,
    ) -> Result<Arc<dyn FileSystem>, ProtocolError> {
        unimplemented!()
    }
}

#[test]
fn the_default_connection_form_is_the_standard_form_on_the_default_port() {
    assert_eq!(Minimal.connection_form(), crate::ConnectionForm::standard(4242));
}

#[test]
fn target_debug_shows_option_keys_but_never_their_values() {
    let target = Target {
        name: "web".into(),
        host: "example.com".into(),
        port: 22,
        username: "deploy".into(),
        password: None,
        options: BTreeMap::from([("token".to_string(), "s3cr3t".to_string())]),
    };

    let printed = format!("{target:?}");

    assert!(printed.contains("token"));
    assert!(!printed.contains("s3cr3t"));
}

fn ssh_invocation() -> ShellInvocation {
    ShellInvocation {
        program: "ssh".into(),
        args: vec!["-p".into(), "22".into(), "deploy@example.com".into()],
        env: vec![("SSHPASS".into(), "hunter2".into())],
    }
}

fn args_of(invocation: &ShellInvocation) -> Vec<String> {
    invocation.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[test]
fn starting_in_a_directory_asks_for_a_terminal_and_changes_into_it_before_the_shell() {
    let started = ssh_invocation().starting_in(&VfsPath::new("/home/tester/docs"));

    assert_eq!(args_of(&started), ["-p", "22", "-t", "deploy@example.com", "cd '/home/tester/docs' && exec $SHELL -l"]);
    assert_eq!(started.program, "ssh");
    assert_eq!(started.env, ssh_invocation().env);
}

#[test]
fn starting_in_a_directory_quotes_spaces_and_apostrophes() {
    let started = ssh_invocation().starting_in(&VfsPath::new("/srv/it's mine; rm -rf"));

    assert_eq!(args_of(&started).last().unwrap(), "cd '/srv/it'\\''s mine; rm -rf' && exec $SHELL -l");
}

#[test]
fn starting_in_a_directory_keeps_a_sshpass_wrapper_in_front() {
    let wrapped = ShellInvocation {
        program: "/usr/bin/sshpass".into(),
        args: vec!["-e".into(), "ssh".into(), "-p".into(), "22".into(), "deploy@example.com".into()],
        env: Vec::new(),
    };

    let started = wrapped.starting_in(&VfsPath::new("/x"));

    assert_eq!(args_of(&started), ["-e", "ssh", "-p", "22", "-t", "deploy@example.com", "cd '/x' && exec $SHELL -l"]);
}

#[test]
fn an_invocation_without_arguments_is_left_as_it_was() {
    let bare = ShellInvocation { program: "ssh".into(), args: Vec::new(), env: Vec::new() };

    assert_eq!(bare.clone().starting_in(&VfsPath::new("/x")), bare);
}

#[test]
fn home_wins_over_the_user_profile() {
    let home = Environment::home_from(Some("/home/me".into()), Some(r"C:\Users\me".into()));

    assert_eq!(home, Some(PathBuf::from("/home/me")));
}

#[test]
fn home_falls_back_to_the_user_profile_when_home_is_unset() {
    let home = Environment::home_from(None, Some(r"C:\Users\me".into()));

    assert_eq!(home, Some(PathBuf::from(r"C:\Users\me")));
}

#[test]
fn an_empty_home_is_ignored() {
    let home = Environment::home_from(Some("".into()), Some(r"C:\Users\me".into()));

    assert_eq!(home, Some(PathBuf::from(r"C:\Users\me")));
}

#[test]
fn without_home_or_user_profile_there_is_no_home() {
    assert_eq!(Environment::home_from(None, None), None);
    assert_eq!(Environment::home_from(Some("".into()), Some("".into())), None);
}

#[test]
fn the_user_name_falls_back_to_username_when_user_is_unset() {
    assert_eq!(Environment::user_from(Some("me".into()), Some("WINME".into())), Some("me".to_string()));
    assert_eq!(Environment::user_from(None, Some("WINME".into())), Some("WINME".to_string()));
    assert_eq!(Environment::user_from(Some(String::new()), Some("WINME".into())), Some("WINME".to_string()));
    assert_eq!(Environment::user_from(None, None), None);
}

fn windows_style(path: &[&std::path::Path]) -> Environment {
    Environment {
        path: Some(std::env::join_paths(path).unwrap()),
        path_extensions: vec![".exe".to_string(), ".cmd".to_string()],
        ..Environment::default()
    }
}

fn touch(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"").unwrap();
}

#[test]
fn a_bare_name_is_found_with_an_extension() {
    let dir = tempfile::tempdir().unwrap();
    touch(&dir.path().join("code.cmd"));

    assert_eq!(windows_style(&[dir.path()]).find_program("code"), Some(dir.path().join("code.cmd")));
}

#[test]
fn extension_order_decides_between_two_matches() {
    let dir = tempfile::tempdir().unwrap();
    touch(&dir.path().join("tool.cmd"));
    touch(&dir.path().join("tool.exe"));

    assert_eq!(windows_style(&[dir.path()]).find_program("tool"), Some(dir.path().join("tool.exe")));
}

#[test]
fn a_name_that_already_has_a_listed_extension_is_found_as_is() {
    let dir = tempfile::tempdir().unwrap();
    touch(&dir.path().join("code.cmd"));

    assert_eq!(windows_style(&[dir.path()]).find_program("code.cmd"), Some(dir.path().join("code.cmd")));
}

#[test]
fn an_earlier_path_directory_wins() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    touch(&first.path().join("tool.exe"));
    touch(&second.path().join("tool.exe"));

    assert_eq!(windows_style(&[first.path(), second.path()]).find_program("tool"), Some(first.path().join("tool.exe")));
}

#[test]
fn a_name_with_a_separator_is_not_searched_on_path() {
    let dir = tempfile::tempdir().unwrap();
    touch(&dir.path().join("sub").join("tool.exe"));
    let env = windows_style(&[dir.path()]);

    assert_eq!(env.find_program("sub/tool"), None);
    assert_eq!(
        env.find_program(dir.path().join("sub").join("tool").to_str().unwrap()),
        Some(dir.path().join("sub").join("tool.exe"))
    );
}

#[test]
fn nothing_listed_means_exact_name_only() {
    let dir = tempfile::tempdir().unwrap();
    touch(&dir.path().join("vi"));
    let env = Environment { path: Some(dir.path().as_os_str().to_owned()), ..Environment::default() };

    assert_eq!(env.find_program("vi"), Some(dir.path().join("vi")));
    assert_eq!(env.find_program("vim"), None);
}

#[test]
fn a_missing_program_or_path_is_none() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(windows_style(&[dir.path()]).find_program("nosuch"), None);
    assert_eq!(Environment::default().find_program("tool"), None);
}

#[test]
fn pathext_text_is_normalised() {
    assert_eq!(Environment::path_extensions_from(Some(".COM;exe;;.Cmd".into())), [".com", ".exe", ".cmd"]);
    assert_eq!(Environment::path_extensions_from(Some(".VBS;.exe;.JS;.cmd;.py".into())), [".exe", ".cmd"]);
    assert_eq!(Environment::path_extensions_from(Some(".vbs;.js".into())), [".com", ".exe", ".bat", ".cmd"]);
    assert_eq!(Environment::path_extensions_from(None), [".com", ".exe", ".bat", ".cmd"]);
    assert_eq!(Environment::path_extensions_from(Some("".into())), [".com", ".exe", ".bat", ".cmd"]);
}

#[test]
fn an_empty_path_entry_never_searches_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = std::env::join_paths([std::path::Path::new(""), dir.path()]).unwrap();
    let env = Environment { path: Some(path), ..Environment::default() };

    assert_eq!(env.find_program("Cargo.toml"), None);
}

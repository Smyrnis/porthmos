use std::{collections::BTreeMap, ffi::OsString, fs, path::PathBuf};

use super::*;

fn sample_target() -> Target {
    let mut options = BTreeMap::new();
    options.insert("identity_file".to_string(), "/home/user/.ssh/id_ed25519".to_string());
    Target {
        name: "production".to_string(),
        host: "server.example.com".to_string(),
        port: 2222,
        username: "deploy".to_string(),
        password: None,
        options,
    }
}

fn args_of(invocation: &ShellInvocation) -> Vec<String> {
    invocation.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[test]
fn command_includes_port_identity_and_user_at_host() {
    let invocation = invocation_for(&sample_target(), None, PasswordLogin::Plain);

    assert_eq!(invocation.program, "ssh");
    assert_eq!(
        args_of(&invocation),
        vec!["-p", "2222", "-i", "/home/user/.ssh/id_ed25519", "deploy@server.example.com",]
    );
}

#[test]
fn command_omits_identity_flag_when_none_is_set() {
    let mut target = sample_target();
    target.options.clear();

    let invocation = invocation_for(&target, None, PasswordLogin::Plain);

    assert_eq!(args_of(&invocation), vec!["-p", "2222", "deploy@server.example.com"]);
}

#[test]
fn command_wraps_with_sshpass_when_password_and_sshpass_are_both_present() {
    let mut target = sample_target();
    target.password = Some("hunter2".to_string());

    let invocation = invocation_for(&target, None, PasswordLogin::Sshpass(PathBuf::from("/usr/bin/sshpass")));

    assert_eq!(invocation.program, "/usr/bin/sshpass");
    assert_eq!(
        args_of(&invocation),
        vec!["-e", "ssh", "-p", "2222", "-i", "/home/user/.ssh/id_ed25519", "deploy@server.example.com",]
    );
    assert_eq!(invocation.env, vec![(OsString::from("SSHPASS"), OsString::from("hunter2"))]);
}

#[test]
fn command_falls_back_to_plain_ssh_when_sshpass_is_not_found() {
    let mut target = sample_target();
    target.password = Some("hunter2".to_string());

    let invocation = invocation_for(&target, None, PasswordLogin::Plain);

    assert_eq!(invocation.program, "ssh");
    assert!(invocation.env.is_empty());
}

#[test]
fn command_falls_back_to_plain_ssh_when_there_is_no_password() {
    let invocation = invocation_for(&sample_target(), None, PasswordLogin::Sshpass(PathBuf::from("/usr/bin/sshpass")));

    assert_eq!(invocation.program, "ssh");
    assert!(invocation.env.is_empty());
}

#[test]
fn command_for_an_ssh_config_alias_pins_the_host_port_user_and_identity() {
    let invocation = invocation_for(&sample_target(), Some("production"), PasswordLogin::Plain);

    assert_eq!(invocation.program, "ssh");
    assert_eq!(
        args_of(&invocation),
        vec![
            "-o",
            "HostName=server.example.com",
            "-p",
            "2222",
            "-l",
            "deploy",
            "-i",
            "/home/user/.ssh/id_ed25519",
            "production"
        ]
    );
}

#[test]
fn command_for_an_ssh_config_alias_still_wraps_with_sshpass_when_a_password_is_saved() {
    let mut target = sample_target();
    target.password = Some("hunter2".to_string());

    let invocation =
        invocation_for(&target, Some("production"), PasswordLogin::Sshpass(PathBuf::from("/usr/bin/sshpass")));

    assert_eq!(invocation.program, "/usr/bin/sshpass");
    assert_eq!(args_of(&invocation)[..2], ["-e", "ssh"]);
    assert_eq!(args_of(&invocation).last().map(String::as_str), Some("production"));
    assert_eq!(invocation.env, vec![(OsString::from("SSHPASS"), OsString::from("hunter2"))]);
}

#[cfg(unix)]
#[test]
fn find_sshpass_in_locates_an_executable_on_the_path() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let sshpass_path = dir.path().join("sshpass");
    fs::write(&sshpass_path, b"").unwrap();
    fs::set_permissions(&sshpass_path, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(find_sshpass_in(dir.path().as_os_str()), Some(sshpass_path));
}

#[test]
fn find_sshpass_in_returns_none_when_not_present() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(find_sshpass_in(dir.path().as_os_str()), None);
}

#[cfg(unix)]
#[test]
fn find_sshpass_in_ignores_a_non_executable_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let sshpass_path = dir.path().join("sshpass");
    fs::write(&sshpass_path, b"").unwrap();
    fs::set_permissions(&sshpass_path, fs::Permissions::from_mode(0o644)).unwrap();

    assert_eq!(find_sshpass_in(dir.path().as_os_str()), None);
}

#[cfg(windows)]
#[test]
fn sshpass_exe_on_the_path_is_never_used() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("sshpass.exe"), b"").unwrap();
    fs::write(dir.path().join("sshpass"), b"").unwrap();

    assert_eq!(find_sshpass_in(dir.path().as_os_str()), None);
}

fn askpass_login() -> PasswordLogin {
    PasswordLogin::Askpass { helper: PathBuf::from("/opt/porthmos/porthmos") }
}

fn target_with_password(password: &str) -> Target {
    let mut target = sample_target();
    target.password = Some(password.to_string());
    target
}

fn env_of(invocation: &ShellInvocation) -> Vec<(String, String)> {
    invocation
        .env
        .iter()
        .map(|(key, value)| (key.to_string_lossy().into_owned(), value.to_string_lossy().into_owned()))
        .collect()
}

fn sshpass_path() -> Option<PathBuf> {
    Some(PathBuf::from("/usr/bin/sshpass"))
}

fn helper_path() -> Option<PathBuf> {
    Some(PathBuf::from("/opt/porthmos/porthmos"))
}

const V8_4: Option<SshVersion> = Some(SshVersion { major: 8, minor: 4 });
const V8_3: Option<SshVersion> = Some(SshVersion { major: 8, minor: 3 });

#[test]
fn a_usable_password_a_helper_and_a_new_enough_ssh_choose_askpass() {
    let login = password_login(Some("hunter2"), helper_path(), V8_4, sshpass_path());

    assert_eq!(login, askpass_login());
}

#[test]
fn an_old_or_unknown_ssh_keeps_sshpass_when_it_is_installed() {
    for version in [V8_3, None] {
        assert_eq!(
            password_login(Some("hunter2"), helper_path(), version, sshpass_path()),
            PasswordLogin::Sshpass(PathBuf::from("/usr/bin/sshpass"))
        );
        assert_eq!(password_login(Some("hunter2"), helper_path(), version, None), PasswordLogin::Plain);
    }
}

#[test]
fn a_password_askpass_cannot_carry_falls_back() {
    for password in ["two\nlines".to_string(), "a".repeat(1024), String::new()] {
        assert_eq!(
            password_login(Some(&password), helper_path(), V8_4, sshpass_path()),
            PasswordLogin::Sshpass(PathBuf::from("/usr/bin/sshpass"))
        );
        assert_eq!(password_login(Some(&password), helper_path(), V8_4, None), PasswordLogin::Plain);
    }
}

#[test]
fn without_a_password_the_login_is_plain_whatever_is_available() {
    assert_eq!(password_login(None, helper_path(), V8_4, sshpass_path()), PasswordLogin::Plain);
}

#[test]
fn without_a_helper_path_askpass_is_not_chosen() {
    assert_eq!(
        password_login(Some("hunter2"), None, V8_4, sshpass_path()),
        PasswordLogin::Sshpass(PathBuf::from("/usr/bin/sshpass"))
    );
    assert_eq!(password_login(Some("hunter2"), None, V8_4, None), PasswordLogin::Plain);
}

#[test]
fn askpass_runs_plain_ssh_with_one_password_try_and_the_helper_environment() {
    let invocation = invocation_for(&target_with_password("hunter2"), None, askpass_login());

    assert_eq!(invocation.program, "ssh");
    assert_eq!(
        args_of(&invocation),
        vec![
            "-o",
            "NumberOfPasswordPrompts=1",
            "-p",
            "2222",
            "-i",
            "/home/user/.ssh/id_ed25519",
            "deploy@server.example.com"
        ]
    );
    assert_eq!(
        env_of(&invocation),
        vec![
            ("SSH_ASKPASS".to_string(), "/opt/porthmos/porthmos".to_string()),
            ("SSH_ASKPASS_REQUIRE".to_string(), "force".to_string()),
            ("PORTHMOS_ASKPASS".to_string(), "1".to_string()),
            ("PORTHMOS_ASKPASS_USER".to_string(), "deploy".to_string()),
            ("PORTHMOS_ASKPASS_PASSWORD".to_string(), "hunter2".to_string()),
        ]
    );
}

#[test]
fn askpass_never_puts_the_password_in_an_argument() {
    let password = "s3cret-\"quoted\" $HOME";
    for alias in [None, Some("production")] {
        let invocation = invocation_for(&target_with_password(password), alias, askpass_login());

        assert!(invocation.args.iter().all(|arg| !arg.to_string_lossy().contains("s3cret")));
        assert!(!invocation.program.to_string_lossy().contains("s3cret"));
    }
}

#[test]
fn askpass_with_an_ssh_config_alias_keeps_the_alias_arguments() {
    let invocation = invocation_for(&target_with_password("hunter2"), Some("production"), askpass_login());

    assert_eq!(args_of(&invocation)[..2], ["-o", "NumberOfPasswordPrompts=1"]);
    assert_eq!(args_of(&invocation).last().map(String::as_str), Some("production"));
    assert!(args_of(&invocation).contains(&"HostName=server.example.com".to_string()));
}

#[test]
fn askpass_without_a_saved_password_is_plain_ssh() {
    let invocation = invocation_for(&sample_target(), None, askpass_login());

    assert!(invocation.env.is_empty());
    assert!(!args_of(&invocation).contains(&"NumberOfPasswordPrompts=1".to_string()));
}

#[test]
fn starting_in_a_folder_still_rewrites_the_askpass_invocation() {
    let invocation = invocation_for(&target_with_password("hunter2"), None, askpass_login())
        .starting_in(&porthmos_vfs::VfsPath::new("/srv/it's"));

    let args = args_of(&invocation);
    let tail = &args[args.len() - 3..];
    assert_eq!(tail[0], "-t");
    assert_eq!(tail[1], "deploy@server.example.com");
    assert_eq!(tail[2], "cd '/srv/it'\\''s' && exec $SHELL -l");
    assert_eq!(args[..2], ["-o", "NumberOfPasswordPrompts=1"]);
}

#[test]
fn the_debug_output_of_an_askpass_invocation_hides_the_password() {
    let invocation = invocation_for(&target_with_password("hunter2-secret"), None, askpass_login());

    assert!(!format!("{invocation:?}").contains("hunter2-secret"));
}

#[test]
fn a_helper_path_that_no_longer_exists_is_not_used() {
    let dir = tempfile::tempdir().unwrap();
    let present = dir.path().join("porthmos");
    fs::write(&present, b"").unwrap();

    assert_eq!(live_helper(Some(present.clone())), Some(present));
    assert_eq!(live_helper(Some(dir.path().join("porthmos (deleted)"))), None);
    assert_eq!(live_helper(Some(dir.path().to_path_buf())), None);
    assert_eq!(live_helper(None), None);
}

#[test]
fn command_passes_a_jump_host_before_the_destination() {
    let mut target = sample_target();
    target.options.insert("proxy_jump".to_string(), "alice@bastion:2222".to_string());

    let invocation = invocation_for(&target, None, PasswordLogin::Plain);

    assert_eq!(
        args_of(&invocation),
        vec!["-p", "2222", "-i", "/home/user/.ssh/id_ed25519", "-J", "alice@bastion:2222", "deploy@server.example.com"]
    );
}

#[test]
fn command_adds_no_jump_flag_for_none_or_blank() {
    for value in ["none", "NONE", "", "  "] {
        let mut target = sample_target();
        target.options.insert("proxy_jump".to_string(), value.to_string());

        let invocation = invocation_for(&target, None, PasswordLogin::Plain);

        assert!(!args_of(&invocation).contains(&"-J".to_string()), "{value:?}");
    }
}

#[test]
fn command_for_an_ssh_config_alias_leaves_the_jump_to_the_config() {
    let mut target = sample_target();
    target.options.insert("proxy_jump".to_string(), "bastion".to_string());

    let invocation = invocation_for(&target, Some("production"), PasswordLogin::Plain);

    assert!(!args_of(&invocation).contains(&"-J".to_string()));
}

#[test]
fn command_passes_a_proxy_command_to_ssh_as_an_option() {
    let mut target = sample_target();
    target.options.insert("proxy_command".to_string(), "corp-gate %h %p".to_string());

    let invocation = invocation_for(&target, None, PasswordLogin::Plain);

    assert_eq!(
        args_of(&invocation),
        vec![
            "-p",
            "2222",
            "-i",
            "/home/user/.ssh/id_ed25519",
            "-o",
            "ProxyCommand=corp-gate %h %p",
            "deploy@server.example.com"
        ]
    );
}

#[test]
fn command_overrides_a_config_proxy_command_when_the_field_says_none() {
    for value in ["none", "NONE", " none "] {
        let mut target = sample_target();
        target.options.insert("proxy_command".to_string(), value.to_string());

        let args = args_of(&invocation_for(&target, None, PasswordLogin::Plain));

        assert!(args.contains(&"ProxyCommand=none".to_string()), "{value:?}: {args:?}");
    }
}

#[test]
fn command_adds_no_proxy_option_for_a_blank_field() {
    for value in ["", "  "] {
        let mut target = sample_target();
        target.options.insert("proxy_command".to_string(), value.to_string());

        let invocation = invocation_for(&target, None, PasswordLogin::Plain);

        assert!(!args_of(&invocation).iter().any(|arg| arg.starts_with("ProxyCommand")), "{value:?}");
    }
}

#[test]
fn command_for_an_ssh_config_alias_leaves_the_proxy_command_to_the_config() {
    let mut target = sample_target();
    target.options.insert("proxy_command".to_string(), "corp-gate %h %p".to_string());

    let invocation = invocation_for(&target, Some("production"), PasswordLogin::Plain);

    assert!(!args_of(&invocation).iter().any(|arg| arg.starts_with("ProxyCommand")));
}

#[test]
fn command_turns_a_proxy_address_into_a_netcat_proxy_command() {
    for (address, expected) in [
        ("socks5://gate:1080", "ProxyCommand=nc -X 5 -x gate:1080 %h %p"),
        ("socks4://gate:1081", "ProxyCommand=nc -X 4 -x gate:1081 %h %p"),
        ("http://gate:3128", "ProxyCommand=nc -X connect -x gate:3128 %h %p"),
        ("socks5://gate", "ProxyCommand=nc -X 5 -x gate:1080 %h %p"),
    ] {
        let mut target = sample_target();
        target.options.insert("proxy".to_string(), address.to_string());

        let invocation = invocation_for(&target, None, PasswordLogin::Plain);

        assert!(args_of(&invocation).contains(&expected.to_string()), "{address}: {:?}", args_of(&invocation));
    }
}

#[test]
fn command_never_connects_directly_when_the_proxy_cannot_be_used() {
    for options in [
        vec![("proxy", "gate:1080")],
        vec![("proxy", "socks5://alice:secret@gate")],
        vec![("proxy", "socks5://gate"), ("proxy_command", "nc %h %p")],
    ] {
        let mut target = sample_target();
        for (key, value) in options {
            target.options.insert(key.to_string(), value.to_string());
        }

        let invocation = invocation_for(&target, None, PasswordLogin::Plain);

        let args = args_of(&invocation);
        assert!(args.contains(&"ProxyCommand=false".to_string()), "{args:?}");
        assert!(!args.iter().any(|arg| arg.contains("secret")), "{args:?}");
    }
}

#[test]
fn a_none_proxy_command_does_not_get_in_the_way_of_a_jump_host() {
    let mut target = sample_target();
    target.options.insert("proxy_jump".to_string(), "bastion".to_string());
    target.options.insert("proxy_command".to_string(), "none".to_string());

    let args = args_of(&invocation_for(&target, None, PasswordLogin::Plain));

    assert!(args.contains(&"-J".to_string()), "{args:?}");
    assert!(!args.iter().any(|arg| arg.starts_with("ProxyCommand")), "{args:?}");
}

#[test]
fn a_proxy_together_with_a_jump_host_never_connects_directly() {
    for (key, value) in [("proxy_command", "corp-gate %h %p"), ("proxy", "socks5://gate:1080")] {
        let mut target = sample_target();
        target.options.insert("proxy_jump".to_string(), "bastion".to_string());
        target.options.insert(key.to_string(), value.to_string());

        let args = args_of(&invocation_for(&target, None, PasswordLogin::Plain));

        assert!(args.contains(&"ProxyCommand=false".to_string()), "{key}: {args:?}");
        assert!(!args.contains(&"-J".to_string()), "{key}: {args:?}");
    }
}

use std::collections::BTreeMap;

use porthmos_vfs::Target;

use super::{Login, ProxyKind, Route, parse_proxy, route_for};
use crate::ssh_config::SshConfigHost;

fn proxy(kind: ProxyKind, host: &str, port: u16) -> Route {
    Route::Proxy { kind, host: host.to_string(), port, login: None }
}

#[test]
fn each_scheme_picks_its_protocol() {
    assert_eq!(parse_proxy("socks5://gate:1080"), Ok(proxy(ProxyKind::Socks5, "gate", 1080)));
    assert_eq!(parse_proxy("socks5h://gate:1080"), Ok(proxy(ProxyKind::Socks5, "gate", 1080)));
    assert_eq!(parse_proxy("socks4://gate:1081"), Ok(proxy(ProxyKind::Socks4, "gate", 1081)));
    assert_eq!(parse_proxy("socks4a://gate:1081"), Ok(proxy(ProxyKind::Socks4, "gate", 1081)));
    assert_eq!(parse_proxy("http://gate:3128"), Ok(proxy(ProxyKind::Http, "gate", 3128)));
}

#[test]
fn a_missing_port_takes_the_usual_one() {
    assert_eq!(parse_proxy("socks5://gate"), Ok(proxy(ProxyKind::Socks5, "gate", 1080)));
    assert_eq!(parse_proxy("socks4://gate"), Ok(proxy(ProxyKind::Socks4, "gate", 1080)));
    assert_eq!(parse_proxy("http://gate"), Ok(proxy(ProxyKind::Http, "gate", 8080)));
}

#[test]
fn the_scheme_and_a_trailing_slash_are_forgiving() {
    assert_eq!(parse_proxy("  SOCKS5://Gate:1080/ "), Ok(proxy(ProxyKind::Socks5, "Gate", 1080)));
}

#[test]
fn an_ipv6_proxy_address_is_written_in_brackets() {
    assert_eq!(parse_proxy("socks5://[::1]:1080"), Ok(proxy(ProxyKind::Socks5, "::1", 1080)));
}

#[test]
fn a_proxy_without_a_scheme_says_which_ones_exist() {
    let error = parse_proxy("gate:1080").unwrap_err();

    assert!(error.contains("http://") && error.contains("socks5://"), "{error}");
}

#[test]
fn schemes_that_are_not_supported_are_named() {
    for text in ["https://gate", "ftp://gate", "ssh://gate"] {
        let error = parse_proxy(text).unwrap_err();

        assert!(error.contains("not supported") || error.contains("http://"), "{text}: {error}");
    }
}

#[test]
fn a_login_in_the_address_is_refused_so_no_password_is_saved_in_a_profile() {
    for text in ["socks5://user:secret@gate:1080", "http://user@gate", "socks4://@gate"] {
        let error = parse_proxy(text).unwrap_err();

        assert!(error.contains("Proxy user") && error.contains("Proxy password"), "{text}: {error}");
        assert!(!error.contains("secret"), "{text}: {error}");
    }
}

#[test]
fn a_path_an_empty_host_or_a_bad_port_is_an_error() {
    for text in
        ["http://gate/extra", "socks5://", "socks5://:1080", "http://gate:0", "http://gate:abc", "http://gate:99999"]
    {
        assert!(parse_proxy(text).is_err(), "{text}");
    }
}

fn target(options: &[(&str, &str)]) -> Target {
    Target {
        name: "box".to_string(),
        host: "box.example".to_string(),
        port: 22,
        username: "u".to_string(),
        password: None,
        options: options.iter().map(|(key, value)| (key.to_string(), value.to_string())).collect::<BTreeMap<_, _>>(),
    }
}

fn alias_with_command(command: &str) -> SshConfigHost {
    SshConfigHost {
        name: "box".to_string(),
        host_name: Some("box.example".to_string()),
        user: None,
        port: None,
        identity_file: None,
        proxy_jump: None,
        proxy_command: Some(command.to_string()),
    }
}

#[test]
fn a_target_with_no_proxy_settings_connects_directly() {
    assert_eq!(route_for(&target(&[]), &[]), Ok(None));
}

#[test]
fn a_proxy_command_on_the_target_is_used() {
    let route = route_for(&target(&[("proxy_command", "nc %h %p")]), &[]);

    assert_eq!(route, Ok(Some(Route::Command("nc %h %p".to_string()))));
}

#[test]
fn a_proxy_on_the_target_is_used() {
    let route = route_for(&target(&[("proxy", "socks5://gate:1080")]), &[]);

    assert_eq!(route, Ok(Some(proxy(ProxyKind::Socks5, "gate", 1080))));
}

#[test]
fn both_a_proxy_and_a_proxy_command_is_an_error() {
    let error = route_for(&target(&[("proxy", "socks5://gate"), ("proxy_command", "nc %h %p")]), &[]).unwrap_err();

    assert!(error.contains("not both"), "{error}");
}

#[test]
fn a_bad_proxy_address_is_reported_without_repeating_it() {
    let error = route_for(&target(&[("proxy", "gate:1080")]), &[]).unwrap_err();

    assert!(error.starts_with("Can't use the proxy setting:"), "{error}");
    assert!(!error.contains("gate:1080"), "{error}");
}

#[test]
fn a_password_typed_into_the_proxy_address_never_appears_in_the_error() {
    for text in ["socks5://alice:hunter2@gate", "http://hunter2@gate", "ftp://alice:hunter2@gate", "gate:hunter2@x"] {
        let error = route_for(&target(&[("proxy", text)]), &[]).unwrap_err();

        assert!(!error.contains("hunter2"), "{text}: {error}");
    }
}

#[test]
fn a_target_with_no_setting_inherits_the_command_of_its_config_alias() {
    let hosts = [alias_with_command("corp-gate %h %p")];

    assert_eq!(route_for(&target(&[]), &hosts), Ok(Some(Route::Command("corp-gate %h %p".to_string()))));
}

#[test]
fn an_alias_for_another_server_is_not_inherited() {
    let mut other = alias_with_command("corp-gate %h %p");
    other.host_name = Some("elsewhere.example".to_string());

    assert_eq!(route_for(&target(&[]), &[other]), Ok(None));
}

#[test]
fn the_word_none_on_the_target_switches_an_inherited_command_off() {
    let hosts = [alias_with_command("corp-gate %h %p")];

    assert_eq!(route_for(&target(&[("proxy_command", "none")]), &hosts), Ok(None));
}

#[test]
fn a_proxy_on_the_target_wins_over_an_inherited_command() {
    let hosts = [alias_with_command("corp-gate %h %p")];

    let route = route_for(&target(&[("proxy", "http://gate:3128")]), &hosts);

    assert_eq!(route, Ok(Some(proxy(ProxyKind::Http, "gate", 3128))));
}

#[test]
fn turning_the_jump_host_off_keeps_the_command_of_the_config_alias() {
    let hosts = [alias_with_command("corp-gate %h %p")];

    let route = route_for(&target(&[("proxy_jump", "none")]), &hosts);

    assert_eq!(route, Ok(Some(Route::Command("corp-gate %h %p".to_string()))));
}

#[test]
fn a_proxy_of_none_does_not_switch_off_the_command_of_the_config_alias() {
    let hosts = [alias_with_command("corp-gate %h %p")];

    let route = route_for(&target(&[("proxy", "none")]), &hosts);

    assert_eq!(route, Ok(Some(Route::Command("corp-gate %h %p".to_string()))));
}

#[test]
fn blank_settings_count_as_no_proxy() {
    assert_eq!(route_for(&target(&[("proxy", "  "), ("proxy_command", "")]), &[]), Ok(None));
}

#[tokio::test]
async fn a_proxy_that_never_answers_is_given_up_on() {
    let (mut client, _silent) = tokio::io::duplex(64);

    let error = super::handshake(ProxyKind::Socks5, &mut client, "h", 22, None, std::time::Duration::from_millis(40))
        .await
        .unwrap_err();

    assert!(error.contains("did not answer in time"), "{error}");
}

#[tokio::test]
async fn each_kind_runs_its_own_handshake() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for (kind, first_byte) in [(ProxyKind::Socks5, 5u8), (ProxyKind::Socks4, 4u8), (ProxyKind::Http, b'C')] {
        let (mut client, mut server) = tokio::io::duplex(256);
        let task = tokio::spawn(async move {
            let mut first = [0u8; 1];
            server.read_exact(&mut first).await.unwrap();
            drop(server);
            first[0]
        });

        let _ = super::handshake(kind, &mut client, "h", 22, None, std::time::Duration::from_secs(1)).await;

        assert_eq!(task.await.unwrap(), first_byte);
        let _ = client.write_all(b"").await;
    }
}

#[test]
fn a_terminal_gets_an_explicit_proxy_command_unchanged() {
    let command = super::terminal_command(&target(&[("proxy_command", "corp-gate %h %p")]));

    assert_eq!(command.as_deref(), Some("corp-gate %h %p"));
}

#[test]
fn a_terminal_gets_nothing_without_proxy_settings() {
    assert_eq!(super::terminal_command(&target(&[])), None);
    assert_eq!(super::terminal_command(&target(&[("proxy", "none"), ("proxy_command", " ")])), None);
}

#[test]
fn a_terminal_proxy_command_for_an_ipv6_proxy_keeps_its_brackets() {
    let command = super::terminal_command(&target(&[("proxy", "socks5://[::1]:1080")]));

    assert!(command.as_deref().is_some_and(|text| text.contains("nc -X 5 -x [::1]:1080 ")), "{command:?}");
}

#[test]
fn a_terminal_never_gets_a_proxy_host_the_shell_would_read_as_code() {
    for host in ["a;touch /tmp/x", "$(id)", "a`id`", "a|b", "a&b", "a b", "-oProxyCommand=x", "a'b", "a\"b", "a>b"] {
        let command = super::terminal_command(&target(&[("proxy", &format!("socks5://{host}:1080"))]));

        assert_eq!(command.as_deref(), Some("false"), "{host}");
    }
}

#[test]
fn a_proxy_together_with_a_jump_host_is_the_way_to_reach_the_first_hop() {
    assert_eq!(
        route_for(&target(&[("proxy", "socks5://gate:1080"), ("proxy_jump", "bastion")]), &[]),
        Ok(Some(Route::Proxy { kind: ProxyKind::Socks5, host: "gate".to_string(), port: 1080, login: None }))
    );
    assert_eq!(
        route_for(&target(&[("proxy_command", "nc %h %p"), ("proxy_jump", "bastion")]), &[]),
        Ok(Some(Route::Command("nc %h %p".to_string())))
    );
}

#[test]
fn a_proxy_together_with_a_jump_host_taken_from_the_config_alias_is_used_too() {
    let mut alias = alias_with_command("unused");
    alias.proxy_command = None;
    alias.proxy_jump = Some("bastion".to_string());

    assert_eq!(
        route_for(&target(&[("proxy", "socks5://gate:1080")]), &[alias]),
        Ok(Some(Route::Proxy { kind: ProxyKind::Socks5, host: "gate".to_string(), port: 1080, login: None }))
    );
}

#[test]
fn a_jump_host_alone_has_no_route_of_its_own() {
    assert_eq!(route_for(&target(&[("proxy_jump", "bastion")]), &[]), Ok(None));
}

#[test]
fn a_command_inherited_from_the_alias_is_not_used_when_the_target_names_its_own_jump_host() {
    let hosts = [alias_with_command("corp-gate %h %p")];

    assert_eq!(route_for(&target(&[("proxy_jump", "bastion")]), &hosts), Ok(None));
}

fn login_of(route: Result<Option<Route>, String>) -> Option<Login> {
    match route {
        Ok(Some(Route::Proxy { login, .. })) => login,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_proxy_user_and_password_become_the_proxy_login() {
    let options = [("proxy", "http://gate:8080"), ("proxy_user", "alice"), ("proxy_password", "secret")];

    let login = login_of(route_for(&target(&options), &[])).unwrap();

    assert_eq!((login.user.as_str(), login.password.as_deref()), ("alice", Some("secret")));
}

#[test]
fn a_proxy_user_alone_is_a_login_without_a_password() {
    let login = login_of(route_for(&target(&[("proxy", "socks5://gate"), ("proxy_user", "alice")]), &[])).unwrap();

    assert_eq!((login.user.as_str(), login.password), ("alice", None));
}

#[test]
fn a_proxy_without_a_user_has_no_login() {
    assert_eq!(login_of(route_for(&target(&[("proxy", "socks5://gate")]), &[])), None);
}

#[test]
fn blank_proxy_user_and_password_count_as_no_login() {
    let options = [("proxy", "socks5://gate"), ("proxy_user", "  "), ("proxy_password", "")];

    assert_eq!(login_of(route_for(&target(&options), &[])), None);
}

#[test]
fn a_proxy_password_without_a_user_is_an_error() {
    let error = route_for(&target(&[("proxy", "http://gate"), ("proxy_password", "secret")]), &[]).unwrap_err();

    assert!(error.contains("Proxy user"), "{error}");
    assert!(!error.contains("secret"), "{error}");
}

#[test]
fn socks4_has_no_password_so_a_password_is_refused() {
    let options = [("proxy", "socks4://gate"), ("proxy_user", "alice"), ("proxy_password", "secret")];

    let error = route_for(&target(&options), &[]).unwrap_err();

    assert!(error.contains("SOCKS4"), "{error}");
    assert!(!error.contains("secret"), "{error}");
}

#[test]
fn a_proxy_login_without_a_proxy_is_an_error() {
    for options in [
        vec![("proxy_user", "alice")],
        vec![("proxy_user", "alice"), ("proxy_command", "nc %h %p")],
        vec![("proxy_password", "secret")],
    ] {
        let error = route_for(&target(&options), &[]).unwrap_err();

        assert!(error.contains("Proxy"), "{error}");
        assert!(!error.contains("secret"), "{error}");
    }
}

#[test]
fn a_proxy_login_next_to_a_proxy_that_is_switched_off_is_ignored() {
    assert_eq!(route_for(&target(&[("proxy", "none"), ("proxy_user", "alice")]), &[]), Ok(None));
}

#[test]
fn the_debug_output_of_a_route_never_shows_the_password() {
    let options = [("proxy", "http://gate"), ("proxy_user", "alice"), ("proxy_password", "hunter2")];

    let shown = format!("{:?}", route_for(&target(&options), &[]));

    assert!(!shown.contains("hunter2"), "{shown}");
}

#[test]
fn a_terminal_cannot_carry_a_proxy_login_so_it_fails_instead_of_connecting_without_it() {
    let options = [("proxy", "socks5://gate:1080"), ("proxy_user", "alice")];

    assert_eq!(super::terminal_command(&target(&options)).as_deref(), Some("false"));
}

#[cfg(unix)]
mod guarded_netcat {
    use std::{os::unix::fs::PermissionsExt, path::Path, process::Command};

    use super::*;

    fn fake_nc(directory: &Path, usage: &str) {
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"-h\" ]; then echo \"{usage}\" >&2; exit 1; fi\necho \"$@\" > '{}'\n",
            directory.join("record").display()
        );
        let path = directory.join("nc");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn run(directory: &Path, address: &str) -> std::process::Output {
        let command = super::super::terminal_command(&target(&[("proxy", address)])).unwrap();
        let command = command.replace("%h", "files.example").replace("%p", "22");
        let path = format!("{}:/usr/bin:/bin", directory.display());
        Command::new("sh").arg("-c").arg(command).env("PATH", path).output().unwrap()
    }

    #[test]
    fn an_openbsd_nc_is_run_with_the_proxy_options_and_the_host_and_port() {
        let directory = tempfile::tempdir().unwrap();
        fake_nc(directory.path(), "usage: nc [-46bCDdFhklNnrStUuvZz] [-X proxy_protocol] [-x proxy_address[:port]]");

        let output = run(directory.path(), "socks5://gate:1080");

        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let seen = std::fs::read_to_string(directory.path().join("record")).unwrap();
        assert_eq!(seen.trim(), "-X 5 -x gate:1080 files.example 22");
    }

    #[test]
    fn a_netcat_without_proxy_support_stops_with_a_message_and_is_never_run() {
        let directory = tempfile::tempdir().unwrap();
        fake_nc(directory.path(), "GNU netcat 0.7.1, a rewrite of the famous networking tool.");

        let output = run(directory.path(), "socks5://gate:1080");

        assert!(!output.status.success());
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(message.contains("OpenBSD") && message.contains("Proxy command"), "{message}");
        assert!(!directory.path().join("record").exists());
    }

    #[test]
    fn the_host_reaches_nc_as_one_argument_even_when_it_has_odd_characters() {
        let directory = tempfile::tempdir().unwrap();
        fake_nc(directory.path(), "usage: nc [-X proxy_protocol]");
        let command = super::super::terminal_command(&target(&[("proxy", "http://gate:3128")])).unwrap();
        let command = command.replace("%h", "'files example'").replace("%p", "22");

        let status = Command::new("sh")
            .arg("-c")
            .arg(command)
            .env("PATH", format!("{}:/usr/bin:/bin", directory.path().display()))
            .status()
            .unwrap();

        assert!(status.success());
        let seen = std::fs::read_to_string(directory.path().join("record")).unwrap();
        assert_eq!(seen.trim(), "-X connect -x gate:3128 files example 22");
    }
}

#[test]
fn a_password_of_only_spaces_is_still_a_password() {
    let options = [("proxy", "http://gate:8080"), ("proxy_user", "alice"), ("proxy_password", "  ")];

    let route = route_for(&target(&options), &[]).unwrap().unwrap();

    let Route::Proxy { login: Some(login), .. } = route else { panic!("{route:?}") };
    assert_eq!(login.password.as_deref(), Some("  "));
}

#[test]
fn a_whitespace_password_does_not_slip_past_the_socks4_refusal() {
    let options = [("proxy", "socks4://gate"), ("proxy_user", "alice"), ("proxy_password", " ")];

    assert!(route_for(&target(&options), &[]).is_err());
}

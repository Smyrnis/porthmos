use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{Tokens, expand as expand_tokens, spawn, spawn_with};

fn expand(command: &str, host: &str, port: u16, user: &str) -> Result<String, String> {
    expand_tokens(command, &Tokens { host, port, user, name: "alias", local_user: "me" })
}

#[test]
fn the_host_port_and_user_replace_their_tokens() {
    assert_eq!(expand("nc %h %p", "files.example", 2222, "alice").unwrap(), "nc files.example 2222");
    assert_eq!(expand("ssh -W %h:%p %r@gate", "10.0.0.5", 22, "bob").unwrap(), "ssh -W 10.0.0.5:22 bob@gate");
}

#[test]
fn a_doubled_percent_is_one_percent() {
    assert_eq!(expand("printf 100%%", "h", 22, "u").unwrap(), "printf 100%");
}

#[test]
fn a_command_without_tokens_is_unchanged() {
    assert_eq!(expand("corp-gate --connect", "h", 22, "u").unwrap(), "corp-gate --connect");
}

#[test]
fn an_unknown_token_is_an_error_naming_it() {
    let error = expand("nc %h %z", "h", 22, "u").unwrap_err();

    assert!(error.contains("%z"), "{error}");
}

#[test]
fn a_trailing_percent_is_an_error() {
    assert!(expand("nc %h 22 %", "h", 22, "u").is_err());
}

#[test]
fn an_ipv6_address_is_allowed_but_a_zone_suffix_is_not() {
    assert_eq!(expand("nc %h %p", "fe80::1", 22, "u").unwrap(), "nc fe80::1 22");
    assert!(expand("nc %h %p", "fe80::1%eth0", 22, "u").unwrap_err().contains("host"));
}

#[test]
fn a_host_the_shell_would_read_as_code_is_refused() {
    for host in ["a;rm -rf ~", "a b", "$(id)", "a`id`", "a|b", "a&b", "a\nb", "a'b", "a\"b", "a>b", ""] {
        let error = expand("nc %h %p", host, 22, "u").unwrap_err();

        assert!(error.contains("host"), "{host:?}: {error}");
    }
}

#[test]
fn a_host_or_user_that_starts_with_a_dash_is_refused_so_it_cannot_become_an_option() {
    for host in ["-oProxyCommand=x", "-w"] {
        let error = expand("ssh -W %h:%p gate", host, 22, "u").unwrap_err();

        assert!(error.contains("host"), "{host}: {error}");
    }
    let error = expand("ssh %r@gate", "h", 22, "-oFoo").unwrap_err();
    assert!(error.contains("user"), "{error}");
}

#[test]
fn a_dash_inside_a_name_is_fine() {
    assert_eq!(expand("nc %h %p", "files-1.example", 22, "a-b").unwrap(), "nc files-1.example 22");
}

#[test]
fn a_user_the_shell_would_read_as_code_is_refused() {
    for user in ["a;b", "a b", "$(id)", "a`id`", "a|b", ""] {
        let error = expand("ssh %r@gate", "h", 22, user).unwrap_err();

        assert!(error.contains("user"), "{user:?}: {error}");
    }
}

#[test]
fn an_unused_unsafe_value_does_not_matter() {
    assert_eq!(expand("gate --go", "a;b", 22, "a b").unwrap(), "gate --go");
}

#[cfg(unix)]
#[tokio::test]
async fn what_is_written_reaches_the_command_and_its_output_comes_back() {
    let (mut stream, _) = spawn("cat").unwrap();

    stream.write_all(b"ping\n").await.unwrap();
    let mut echoed = [0u8; 5];
    stream.read_exact(&mut echoed).await.unwrap();

    assert_eq!(&echoed, b"ping\n");
}

#[cfg(unix)]
#[tokio::test]
async fn a_command_that_ends_closes_the_stream() {
    let (mut stream, _) = spawn("printf done").unwrap();

    let mut all = Vec::new();
    stream.read_to_end(&mut all).await.unwrap();

    assert_eq!(all, b"done");
}

#[cfg(unix)]
#[tokio::test]
async fn what_the_command_prints_as_an_error_is_kept_for_the_message() {
    let (mut stream, diagnostics) = spawn("echo 'no route to gate' >&2; exit 3").unwrap();

    let mut all = Vec::new();
    stream.read_to_end(&mut all).await.unwrap();

    assert_eq!(diagnostics.text().await, "no route to gate");
}

#[cfg(unix)]
#[tokio::test]
async fn error_text_is_cut_short_and_stripped_of_control_characters() {
    let (mut stream, diagnostics) =
        spawn("printf '\\033[31mred\\033[0m\\n'; printf 'x%.0s' $(seq 1 2000) >&2").unwrap();

    let mut all = Vec::new();
    stream.read_to_end(&mut all).await.unwrap();
    let text = diagnostics.text().await;

    assert!(text.len() <= 300, "{}", text.len());
    assert!(!text.contains('\u{1b}'));
}

#[cfg(unix)]
#[tokio::test]
async fn a_command_that_prints_nothing_leaves_no_error_text() {
    let (mut stream, diagnostics) = spawn("true").unwrap();

    let mut all = Vec::new();
    stream.read_to_end(&mut all).await.unwrap();

    assert_eq!(diagnostics.text().await, "");
}

#[cfg(unix)]
#[tokio::test]
async fn dropping_the_stream_stops_the_command() {
    let marker = tempfile::tempdir().unwrap();
    let pid_file = marker.path().join("pid");
    let (stream, _) = spawn(&format!("echo $$ > '{}'; exec sleep 30", pid_file.display())).unwrap();
    let pid = loop {
        if let Ok(text) = std::fs::read_to_string(&pid_file)
            && let Ok(pid) = text.trim().parse::<i32>()
        {
            break pid;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };

    drop(stream);

    let gone = async {
        loop {
            let alive = std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success();
            if !alive {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), gone).await.expect("the command was stopped");
}

#[cfg(unix)]
#[tokio::test]
async fn what_a_command_prints_before_it_stalls_is_still_reported() {
    let (_stream, diagnostics) = spawn("echo 'connection refused' >&2; exec sleep 30").unwrap();

    assert_eq!(diagnostics.text().await, "connection refused");
}

#[test]
fn the_name_and_the_local_user_have_their_own_tokens() {
    let tokens = Tokens { host: "10.0.0.5", port: 22, user: "bob", name: "gw", local_user: "alice" };

    assert_eq!(
        expand_tokens("ssh -W %h:%p -l %r gate.%n.corp # %u", &tokens).unwrap(),
        "ssh -W 10.0.0.5:22 -l bob gate.gw.corp # alice"
    );
}

#[test]
fn a_name_or_local_user_the_shell_would_read_as_code_is_refused() {
    for name in ["a;b", "a b", "$(id)", "-x", ""] {
        let tokens = Tokens { host: "h", port: 22, user: "u", name, local_user: "me" };
        let error = expand_tokens("gate %n", &tokens).unwrap_err();

        assert!(error.contains("name"), "{name:?}: {error}");
    }
    for local in ["a;b", "a b", "-x", ""] {
        let tokens = Tokens { host: "h", port: 22, user: "u", name: "n", local_user: local };
        let error = expand_tokens("gate %u", &tokens).unwrap_err();

        assert!(error.contains("local user"), "{local:?}: {error}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_command_that_sends_nothing_in_time_is_given_up_on_with_what_it_printed() {
    let (mut stream, diagnostics) =
        spawn_with("echo 'waiting for gate' >&2; exec sleep 30", std::time::Duration::from_millis(100)).unwrap();

    let mut byte = [0u8; 1];
    let error = stream.read_exact(&mut byte).await.unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert!(error.to_string().contains("sent nothing"), "{error}");
    assert_eq!(diagnostics.text().await, "waiting for gate");
}

#[cfg(unix)]
#[tokio::test]
async fn the_time_limit_stops_applying_once_the_command_has_sent_something() {
    let (mut stream, _) = spawn_with("printf a; sleep 0.4; printf b", std::time::Duration::from_millis(150)).unwrap();

    let mut both = Vec::new();
    stream.read_to_end(&mut both).await.unwrap();

    assert_eq!(both, b"ab");
}

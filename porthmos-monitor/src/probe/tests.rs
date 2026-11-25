use super::*;

fn token() -> Token {
    Token::from_hex("00112233445566778899aabbccddeeff").unwrap()
}

#[test]
fn probe_command_is_posix_sh_without_bashisms() {
    let script = probe_command(&token());

    for forbidden in ["[[", "<(", ">(", "local ", "$'", "function ", "echo -e", "source ", "=(", "&>", "${!"] {
        assert!(!script.contains(forbidden), "the script uses {forbidden:?}");
    }
    assert!(script.ends_with('\n'));
    assert!(script.len() < 8 * 1024, "{} bytes", script.len());
}

#[test]
fn every_section_marker_appears_once_in_order_and_the_end_marker_closes_both_paths() {
    let script = probe_command(&token());
    let names: Vec<&str> = script
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("m "))
        .map(|rest| rest.split(';').next().unwrap().trim())
        .collect();

    assert_eq!(
        names,
        [
            "os",
            "end",
            "uid",
            "hostname",
            "osrelease",
            "kernel",
            "uptime",
            "loadavg",
            "meminfo",
            "stat",
            "netdev",
            "df",
            "passwd",
            "pagesize",
            "status",
            "cmdline",
            "procs",
            "end"
        ]
    );
}

#[test]
fn a_marker_always_starts_on_its_own_line_even_after_output_that_has_no_final_newline() {
    let script = probe_command(&token());

    assert!(script.contains("m() { printf '\\n@@00112233445566778899aabbccddeeff:%s\\n' \"$1\"; }"), "{script}");
    assert!(!script.contains("echo '@@"));
}

#[test]
fn process_sections_are_attributed_by_the_path_grep_prints_not_by_text_a_process_controls() {
    let script = probe_command(&token());

    assert!(script.contains("grep -aHs '' /proc/[0-9]*/stat"), "{script}");
    assert!(script.contains("grep -aHs '' /proc/[0-9]*/cmdline"), "{script}");
    assert!(script.contains("grep -Hs '^Uid:' /proc/[0-9]*/status"), "{script}");
    assert!(!script.contains("tail -n +1"));
    assert!(!script.contains("cat /proc/[0-9]*"));
}

#[test]
fn the_placeholder_never_survives_and_the_token_is_only_marker_text() {
    let script = probe_command(&token());

    assert!(!script.contains("@@T:"));
    assert_eq!(script.matches("00112233445566778899aabbccddeeff").count(), 1);
}

#[test]
fn new_tokens_are_thirty_two_hex_digits_and_differ() {
    let (first, second) = (Token::new(), Token::new());

    assert_eq!(first.as_str().len(), 32);
    assert!(first.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(first, second);
}

#[test]
fn from_hex_refuses_anything_that_could_reach_a_shell_or_is_too_long() {
    for bad in ["", "zz", "abc'; rm -rf /; '", "ab cd", "ab\ncd", &"a".repeat(65)] {
        assert!(Token::from_hex(bad).is_none(), "{bad:?}");
    }
    assert!(Token::from_hex("ABCDEF0123456789").is_some());
}

#[cfg(target_os = "linux")]
fn run_under(shell: &[&str], token: &Token) -> Option<Vec<u8>> {
    let (program, arguments) = shell.split_first()?;
    let output =
        std::process::Command::new(program).args(arguments).arg("-c").arg(probe_command(token)).output().ok()?;
    output.status.success().then_some(output.stdout)
}

#[cfg(target_os = "linux")]
fn assert_a_real_run_parses(shell: &[&str]) {
    let token = Token::new();
    let Some(stdout) = run_under(shell, &token) else {
        return;
    };

    let crate::Parsed::Linux { snapshot, .. } = crate::parse(&stdout, &token, None, std::time::Duration::from_secs(3))
    else {
        panic!("a Linux host should parse as Linux");
    };

    assert!(snapshot.host.hostname.is_some(), "{shell:?}");
    assert!(snapshot.memory.is_some_and(|memory| memory.total > 0), "{shell:?}");
    assert!(snapshot.load.is_some(), "{shell:?}");
    assert!(snapshot.cpu.cores.is_some_and(|cores| cores >= 1), "{shell:?}");
    assert!(snapshot.host.uptime_seconds.is_some(), "{shell:?}");
    assert!(!snapshot.processes.is_empty(), "{shell:?} lost the process table: {:?}", snapshot.problems);
}

#[cfg(target_os = "linux")]
#[test]
fn the_script_runs_under_the_local_sh_and_its_output_parses() {
    assert_a_real_run_parses(&["sh"]);
}

#[cfg(target_os = "linux")]
#[test]
fn the_script_runs_under_dash_and_busybox_when_they_are_installed() {
    assert_a_real_run_parses(&["dash"]);
    assert_a_real_run_parses(&["busybox", "sh"]);
}

#[cfg(target_os = "linux")]
#[test]
fn a_process_that_puts_a_newline_in_its_own_name_cannot_forge_another_process_row() {
    let mut forger = std::process::Command::new("sh")
        .arg("-c")
        .arg("printf 'a\\n4194303 (evil' > /proc/$$/comm && sleep 20; true")
        .spawn()
        .expect("sh starts");
    std::thread::sleep(std::time::Duration::from_millis(300));
    let token = Token::new();
    let output = run_under(&["sh"], &token);
    let _ = forger.kill();
    let _ = forger.wait();
    let Some(stdout) = output else {
        return;
    };

    let crate::Parsed::Linux { snapshot, .. } = crate::parse(&stdout, &token, None, std::time::Duration::from_secs(3))
    else {
        panic!("a Linux host should parse as Linux");
    };

    assert!(
        snapshot.processes.iter().all(|process| process.name != "evil" && process.pid != 4_194_303),
        "a forged row was accepted"
    );
}

use std::process::Command;

fn porthmos(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_porthmos")).args(args).output().unwrap()
}

#[test]
fn version_prints_one_line_and_exits_zero() {
    let output = porthmos(&["--version"]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("porthmos {}\n", env!("CARGO_PKG_VERSION")));
    assert!(output.stderr.is_empty());
}

#[test]
fn an_unknown_option_exits_with_code_two_and_a_hint_on_stderr() {
    let output = porthmos(&["--bogus"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown option '--bogus'") && stderr.contains("porthmos --help"), "{stderr}");
}

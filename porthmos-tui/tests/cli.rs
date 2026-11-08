use std::process::Command;

fn porthmos(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_porthmos")).args(args).output().unwrap()
}

fn askpass(prompt: &str, variables: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_porthmos"));
    command.arg(prompt).env("PORTHMOS_ASKPASS", "1");
    for (name, value) in variables {
        command.env(name, value);
    }
    command.output().unwrap()
}

const ASKPASS_VARIABLES: [(&str, &str); 2] =
    [("PORTHMOS_ASKPASS_USER", "deploy"), ("PORTHMOS_ASKPASS_PASSWORD", "s3cret")];

#[test]
fn the_helper_mode_prints_only_the_password_for_the_exact_prompt() {
    let output = askpass("deploy@host's password: ", &ASKPASS_VARIABLES);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"s3cret\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn the_helper_mode_refuses_a_passphrase_prompt_with_nothing_on_stdout() {
    let output = askpass("Enter passphrase for key '/home/u/passwords/id': ", &ASKPASS_VARIABLES);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn the_helper_mode_is_off_unless_its_variable_is_exactly_one() {
    for value in ["0", "", "true", "yes"] {
        let output = Command::new(env!("CARGO_BIN_EXE_porthmos"))
            .arg("--version")
            .env("PORTHMOS_ASKPASS", value)
            .envs(ASKPASS_VARIABLES)
            .output()
            .unwrap();

        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("porthmos {}\n", env!("CARGO_PKG_VERSION")));
    }
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

#[test]
fn update_with_a_bad_argument_exits_two_with_nothing_on_stdout() {
    let output = porthmos(&["update", "--bogus"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown option '--bogus'"), "{stderr}");
}

use super::*;

fn interpret_all(args: &[&str]) -> Invocation {
    interpret(args.iter().map(|argument| argument.to_string()))
}

fn version_line() -> String {
    format!("porthmos {}", env!("CARGO_PKG_VERSION"))
}

#[test]
fn no_arguments_runs_the_app() {
    assert_eq!(interpret_all(&[]), Invocation::Run);
}

#[test]
fn a_positional_argument_is_ignored_and_runs_the_app() {
    assert_eq!(interpret_all(&["/some/folder"]), Invocation::Run);
    assert_eq!(interpret_all(&["-"]), Invocation::Run);
}

#[test]
fn version_flags_print_the_package_version() {
    for flag in ["--version", "-V"] {
        assert_eq!(interpret_all(&[flag]), Invocation::Print(version_line()), "{flag}");
    }
}

#[test]
fn help_flags_print_a_usage_that_mentions_both_flags() {
    for flag in ["--help", "-h"] {
        let Invocation::Print(text) = interpret_all(&[flag]) else {
            panic!("{flag} did not print");
        };

        for expected in ["--version", "-V", "--help", "-h", "Usage"] {
            assert!(text.contains(expected), "{flag}: {text}");
        }
    }
}

#[test]
fn an_unknown_option_is_rejected_by_name() {
    assert_eq!(interpret_all(&["--versoin"]), Invocation::Reject("unknown option '--versoin'".to_string()));
    assert_eq!(interpret_all(&["-x"]), Invocation::Reject("unknown option '-x'".to_string()));
}

#[test]
fn arguments_are_examined_in_order_and_the_first_decisive_one_wins() {
    assert_eq!(interpret_all(&["--version", "--bogus"]), Invocation::Print(version_line()));
    assert_eq!(interpret_all(&["--bogus", "--version"]), Invocation::Reject("unknown option '--bogus'".to_string()));
    assert_eq!(interpret_all(&["folder", "--version"]), Invocation::Print(version_line()));
}

#[test]
fn update_is_interpreted_as_a_subcommand() {
    assert_eq!(interpret_all(&["update"]), Invocation::Update { check_only: false });
}

#[test]
fn update_check_sets_check_only() {
    assert_eq!(interpret_all(&["update", "--check"]), Invocation::Update { check_only: true });
}

#[test]
fn an_unknown_option_after_update_is_rejected() {
    assert_eq!(interpret_all(&["update", "--bogus"]), Invocation::Reject("unknown option '--bogus'".to_string()));
}

#[test]
fn an_extra_word_after_update_is_rejected() {
    assert_eq!(interpret_all(&["update", "now"]), Invocation::Reject("unexpected argument 'now'".to_string()));
}

#[test]
fn version_and_help_still_win_after_update() {
    assert_eq!(interpret_all(&["update", "--version"]), Invocation::Print(version_line()));
    assert!(matches!(interpret_all(&["update", "-h"]), Invocation::Print(text) if text.contains("Usage")));
}

#[test]
fn the_usage_lists_the_update_command() {
    let Invocation::Print(text) = interpret_all(&["--help"]) else { panic!("no usage") };

    assert!(text.contains("update") && text.contains("--check"), "{text}");
}

#[test]
fn describe_update_uses_the_exact_texts() {
    use porthmos_core::HeadlessUpdate::*;

    assert_eq!(describe_update(&UpToDate { current: "0.3.0".into() }), "Porthmos 0.3.0 is up to date.");
    assert_eq!(describe_update(&Available { version: "0.4.0".into() }), "Porthmos 0.4.0 is available.");
    assert_eq!(
        describe_update(&Managed { version: "0.4.0".into(), reason: "installed by cargo".into() }),
        "Porthmos 0.4.0 is available. This copy is managed (installed by cargo); update it with your package manager."
    );
    assert_eq!(describe_update(&Installed { version: "0.4.0".into() }), "Porthmos updated to 0.4.0.");
}

#[test]
fn install_message_prints_success_on_stdout_and_failure_on_stderr_and_nothing_otherwise() {
    use porthmos_core::ApplyOutcome;

    assert_eq!(
        install_message(&ApplyOutcome::Installed { version: "0.4.0".into() }),
        Some(ExitLine::Out("Porthmos updated to 0.4.0.".to_string()))
    );
    assert_eq!(
        install_message(&ApplyOutcome::Failed { version: "0.4.0".into(), message: "disk full".into() }),
        Some(ExitLine::Err("porthmos: couldn't install the update to 0.4.0: disk full".to_string()))
    );
    assert_eq!(install_message(&ApplyOutcome::Nothing), None);
}

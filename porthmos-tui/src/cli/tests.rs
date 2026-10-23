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

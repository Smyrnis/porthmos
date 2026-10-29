use super::*;

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

#[test]
fn no_arguments_run_the_window() {
    assert_eq!(interpret(args(&[])), Invocation::Run);
}

#[test]
fn version_flags_print_the_gui_version() {
    let expected = Invocation::Print(format!("porthmos-gui {}", env!("CARGO_PKG_VERSION")));

    assert_eq!(interpret(args(&["--version"])), expected);
    assert_eq!(interpret(args(&["-V"])), expected);
}

#[test]
fn help_lists_version_help_and_update_only() {
    let Invocation::Print(text) = interpret(args(&["--help"])) else { panic!("help prints") };

    assert!(text.contains("--version") && text.contains("--help") && text.contains("update --check"));
    assert!(!text.contains("--connect") && !text.contains("--local"));
    assert_eq!(interpret(args(&["-h"])), Invocation::Print(text));
}

#[test]
fn update_and_update_check_are_recognised() {
    assert_eq!(interpret(args(&["update"])), Invocation::Update { check_only: false });
    assert_eq!(interpret(args(&["update", "--check"])), Invocation::Update { check_only: true });
}

#[test]
fn unknown_options_are_rejected_with_the_option_named() {
    assert_eq!(interpret(args(&["--connect"])), Invocation::Reject("unknown option '--connect'".to_string()));
    assert_eq!(interpret(args(&["-x"])), Invocation::Reject("unknown option '-x'".to_string()));
}

#[test]
fn a_stray_word_is_rejected() {
    assert_eq!(interpret(args(&["sideways"])), Invocation::Reject("unexpected argument 'sideways'".to_string()));
}

#[test]
fn an_option_after_update_is_rejected_unless_it_is_check() {
    assert_eq!(interpret(args(&["update", "--local"])), Invocation::Reject("unknown option '--local'".to_string()));
    assert_eq!(interpret(args(&["update", "now"])), Invocation::Reject("unexpected argument 'now'".to_string()));
}

#[test]
fn describing_an_update_names_the_gui() {
    let up_to_date = HeadlessUpdate::UpToDate { current: "0.4.0".to_string() };

    assert_eq!(describe_update(&up_to_date), "Porthmos 0.4.0 is up to date.");
    assert_eq!(
        describe_update(&HeadlessUpdate::Installed { version: "0.5.0".to_string() }),
        "Porthmos updated to 0.5.0."
    );
}

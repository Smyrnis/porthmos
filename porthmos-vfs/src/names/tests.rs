use super::*;

#[test]
fn permissive_rules_accept_every_name_and_change_nothing() {
    for name in ["a:b", "CON", "x.", "é", "what?"] {
        assert_eq!(NameRules::Permissive.problem(name), None, "{name}");
        assert_eq!(NameRules::Permissive.sanitize(name), name);
        assert_eq!(NameRules::Permissive.fold(name), name);
    }
    assert!(NameRules::Permissive.is_case_sensitive());
}

#[test]
fn windows_rules_reject_each_forbidden_character() {
    for forbidden in ['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\u{1}', '\u{1f}'] {
        let name = format!("a{forbidden}b");

        assert_eq!(NameRules::Windows.problem(&name), Some(NameProblem::ForbiddenCharacter(forbidden)));
    }
}

#[test]
fn windows_rules_reject_reserved_device_names_with_or_without_an_extension() {
    for name in ["CON", "con", "Nul.txt", "COM1", "lpt9.log", "PRN", "aux.tar.gz"] {
        assert!(matches!(NameRules::Windows.problem(name), Some(NameProblem::ReservedDeviceName(_))), "{name}");
    }
    for name in ["COM0", "CONSOLE", "console.txt", "com10", "nulx", "lpt"] {
        assert_eq!(NameRules::Windows.problem(name), None, "{name}");
    }
}

#[test]
fn windows_rules_reject_a_trailing_dot_or_space_and_the_empty_name() {
    assert_eq!(NameRules::Windows.problem("report."), Some(NameProblem::TrailingDotOrSpace));
    assert_eq!(NameRules::Windows.problem("report "), Some(NameProblem::TrailingDotOrSpace));
    assert_eq!(NameRules::Windows.problem(""), Some(NameProblem::Empty));
}

#[test]
fn windows_sanitize_gives_the_documented_names() {
    let cases = [
        ("a:b.txt", "a_b.txt"),
        ("what?.txt", "what_.txt"),
        ("CON", "_CON"),
        ("con.txt", "_con.txt"),
        ("name. ", "name"),
        ("...", "_"),
        ("", "_"),
        ("tab\there", "tab_here"),
        ("fine.txt", "fine.txt"),
    ];
    for (name, expected) in cases {
        assert_eq!(NameRules::Windows.sanitize(name), expected, "{name:?}");
    }
}

#[test]
fn a_sanitized_windows_name_always_passes_the_rules() {
    for name in ["a:b.txt", "what?.txt", "CON", "con.txt", "name. ", "...", "", "tab\there", "a<b>c|d", "NUL.", "x\\y"]
    {
        let once = NameRules::Windows.sanitize(name);

        assert_eq!(NameRules::Windows.problem(&once), None, "{name:?} -> {once:?}");
        assert_eq!(NameRules::Windows.sanitize(&once), once);
    }
}

#[test]
fn windows_fold_ignores_letter_case() {
    assert_eq!(NameRules::Windows.fold("README.md"), NameRules::Windows.fold("readme.MD"));
    assert_eq!(NameRules::Windows.fold("\u{c9}"), NameRules::Windows.fold("\u{e9}"));
    assert!(!NameRules::Windows.is_case_sensitive());
}

#[test]
fn problems_read_as_short_reasons() {
    assert_eq!(NameProblem::ForbiddenCharacter(':').to_string(), "contains ':'");
    assert_eq!(NameProblem::ReservedDeviceName("CON".to_string()).to_string(), "is a reserved device name");
    assert_eq!(NameProblem::TrailingDotOrSpace.to_string(), "ends with a dot or a space");
    assert_eq!(NameProblem::Empty.to_string(), "is empty");
}

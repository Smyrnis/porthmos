use super::name_problem;

#[test]
fn an_empty_name_is_refused() {
    assert!(name_problem("").is_some());
}

#[test]
fn dot_and_dotdot_are_refused() {
    assert!(name_problem(".").is_some());
    assert!(name_problem("..").is_some());
}

#[test]
fn a_slash_is_refused() {
    assert!(name_problem("a/b").unwrap().contains('/'));
}

#[test]
fn control_characters_and_newlines_are_refused() {
    assert!(name_problem("a\nb").is_some());
    assert!(name_problem("a\tb").is_some());
    assert!(name_problem("a\u{7f}").is_some());
}

#[test]
fn leading_or_trailing_spaces_are_refused() {
    assert!(name_problem(" a").is_some());
    assert!(name_problem("a ").is_some());
}

#[test]
fn a_name_longer_than_255_bytes_is_refused() {
    assert!(name_problem(&"a".repeat(255)).is_none());
    assert!(name_problem(&"a".repeat(256)).is_some());
    assert!(name_problem(&"é".repeat(128)).is_some());
}

#[test]
fn windows_reserved_names_are_refused_on_every_platform() {
    for name in ["CON", "nul", "COM1.txt", "aux.", "Lpt9", "prn.tar.gz"] {
        assert!(name_problem(name).is_some(), "{name}");
    }
    assert!(name_problem("console").is_none());
    assert!(name_problem("com10").is_none());
}

#[test]
fn an_ordinary_unicode_name_is_accepted() {
    assert_eq!(name_problem("ünï-文件 with spaces.txt"), None);
    assert_eq!(name_problem(".hidden"), None);
}

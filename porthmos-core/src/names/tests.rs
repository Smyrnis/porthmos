use super::{entry_name_problem, name_problem};

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

fn taken(existing: &'static [&'static str]) -> impl Fn(&str) -> bool {
    move |name| existing.contains(&name)
}

#[test]
fn a_new_name_that_is_free_and_well_formed_has_no_problem() {
    assert_eq!(entry_name_problem("fresh", None, taken(&["old"])), None);
}

#[test]
fn a_badly_formed_name_is_reported_before_a_clash() {
    assert_eq!(entry_name_problem("a/b", None, taken(&["a/b"])), name_problem("a/b"));
}

#[test]
fn a_name_that_already_exists_is_refused_and_quoted() {
    assert_eq!(entry_name_problem("old", None, taken(&["old"])), Some("\"old\" already exists".to_string()));
}

#[test]
fn renaming_to_the_name_it_already_has_is_not_a_problem() {
    assert_eq!(entry_name_problem("old", Some("old"), taken(&["old"])), None);
}

#[test]
fn renaming_to_another_existing_name_is_refused() {
    assert!(entry_name_problem("other", Some("old"), taken(&["old", "other"])).is_some());
}

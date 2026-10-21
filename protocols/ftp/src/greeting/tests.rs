use super::*;

#[test]
fn a_plain_greeting_is_returned_cleaned() {
    assert_eq!(greeting_info(Some("vsFTPd 3.0.5 ready")).as_deref(), Some("vsFTPd 3.0.5 ready"));
}

#[test]
fn a_leading_status_code_is_dropped() {
    assert_eq!(greeting_info(Some("220 ProFTPD Server (Debian)")).as_deref(), Some("ProFTPD Server (Debian)"));
    assert_eq!(greeting_info(Some("220-Welcome to the box")).as_deref(), Some("Welcome to the box"));
}

#[test]
fn only_the_first_line_of_a_multiline_greeting_is_used() {
    assert_eq!(greeting_info(Some("220-Welcome\r\n220-second line\r\n220 ready")).as_deref(), Some("Welcome"));
    assert_eq!(greeting_info(Some("\r\n  \r\nhello\r\nworld")).as_deref(), Some("hello"));
}

#[test]
fn an_empty_or_missing_greeting_is_none() {
    assert_eq!(greeting_info(None), None);
    for text in ["", "   ", "220", "220 ", "\r\n"] {
        assert_eq!(greeting_info(Some(text)), None, "{text:?}");
    }
}

#[test]
fn a_greeting_with_control_characters_is_cleaned_and_cut() {
    assert_eq!(greeting_info(Some("a\u{1b}[2Jb\u{7}c")).as_deref(), Some("a [2Jb c"));
    let long = greeting_info(Some(&"x".repeat(300))).unwrap();
    assert!(long.chars().count() <= 40 && long.ends_with('\u{2026}'), "{long}");
}

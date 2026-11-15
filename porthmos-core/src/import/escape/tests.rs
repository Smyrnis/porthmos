use super::*;

#[test]
fn decodes_spaces_and_percent() {
    assert_eq!(decode("My%20Site%25"), "My Site%");
}

#[test]
fn lowercase_hex_is_accepted() {
    assert_eq!(decode("a%2fb"), "a/b");
}

#[test]
fn utf8_marker_is_stripped_and_text_decoded() {
    assert_eq!(decode("%EF%BB%BF%C3%A9"), "é");
    assert_eq!(decode("%EF%BB%BFcaf%C3%A9"), "café");
}

#[test]
fn slash_is_left_alone() {
    assert_eq!(decode("A/B/Site"), "A/B/Site");
}

#[test]
fn invalid_sequences_stay_literal() {
    assert_eq!(decode("100%"), "100%");
    assert_eq!(decode("%zz"), "%zz");
    assert_eq!(decode("%4"), "%4");
}

#[test]
fn bytes_that_are_not_utf8_do_not_panic() {
    assert_eq!(decode("a%FFb"), "a\u{fffd}b");
}

#[test]
fn plain_text_is_unchanged() {
    assert_eq!(decode("plain-name_1.0"), "plain-name_1.0");
}

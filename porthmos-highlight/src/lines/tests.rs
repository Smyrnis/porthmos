use super::lines_of;

#[test]
fn a_trailing_newline_does_not_add_a_line() {
    assert_eq!(lines_of("a\nb\n"), ["a", "b"]);
}

#[test]
fn carriage_returns_are_stripped_and_a_missing_final_newline_keeps_the_last_line() {
    assert_eq!(lines_of("a\r\nb"), ["a", "b"]);
}

#[test]
fn empty_text_has_no_lines() {
    assert!(lines_of("").is_empty());
}

#[test]
fn a_lone_newline_is_one_empty_line() {
    assert_eq!(lines_of("\n"), [""]);
}

#[test]
fn a_blank_line_before_the_end_is_kept() {
    assert_eq!(lines_of("a\n\n"), ["a", ""]);
}

use super::*;

#[test]
fn take_width_keeps_whole_characters_up_to_the_width() {
    assert_eq!(take_width("abc", 2), "ab");
    assert_eq!(take_width("abc", 10), "abc");
    assert_eq!(take_width("abc", 0), "");
}

#[test]
fn take_width_never_splits_a_double_width_character() {
    assert_eq!(take_width("\u{4e00}\u{4e8c}", 3), "\u{4e00}");
    assert_eq!(take_width("\u{4e00}", 1), "");
}

#[test]
fn truncate_with_ellipsis_leaves_text_that_fits_alone() {
    assert_eq!(truncate_with_ellipsis("abc", 3), "abc");
}

#[test]
fn truncate_with_ellipsis_ends_cut_text_with_an_ellipsis_inside_the_budget() {
    assert_eq!(truncate_with_ellipsis("abcdef", 4), "abc\u{2026}");
    assert_eq!(truncate_with_ellipsis("abcdef", 1), "\u{2026}");
    assert_eq!(truncate_with_ellipsis("\u{4e00}\u{4e8c}\u{4e09}", 4), "\u{4e00}\u{2026}");
}

#[test]
fn truncate_with_ellipsis_with_no_budget_is_empty() {
    assert_eq!(truncate_with_ellipsis("abc", 0), "");
}

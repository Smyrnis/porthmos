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

#[test]
fn truncate_start_leaves_text_that_fits_alone() {
    assert_eq!(truncate_start_with_ellipsis("/home", 10), "/home");
}

#[test]
fn truncate_start_keeps_the_tail_behind_an_ellipsis() {
    assert_eq!(truncate_start_with_ellipsis("/home/user/projects", 9), "\u{2026}projects");
    assert_eq!(truncate_start_with_ellipsis("abcdef", 1), "\u{2026}");
    assert_eq!(truncate_start_with_ellipsis("abcdef", 0), "");
}

#[test]
fn truncate_start_never_splits_a_double_width_character() {
    assert_eq!(truncate_start_with_ellipsis("\u{4e00}\u{4e8c}\u{4e09}", 4), "\u{2026}\u{4e09}");
}

#[test]
fn pad_right_and_pad_left_fill_by_display_width_and_never_cut() {
    assert_eq!(pad_right("ab", 5), "ab   ");
    assert_eq!(pad_left("ab", 5), "   ab");
    assert_eq!(pad_right("\u{65e5}", 4), "\u{65e5}  ");
    assert_eq!(pad_left("\u{65e5}", 4), "  \u{65e5}");
    assert_eq!(pad_right("abcdef", 3), "abcdef");
    assert_eq!(pad_left("abcdef", 3), "abcdef");
}

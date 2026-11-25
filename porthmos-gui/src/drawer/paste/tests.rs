use super::*;

#[test]
fn a_single_line_without_a_newline_needs_no_confirmation() {
    assert!(!needs_paste_confirmation("ls -la", false));
    assert!(!needs_paste_confirmation("", false));
}

#[test]
fn any_newline_needs_confirmation_when_the_shell_would_run_it() {
    for text in ["ls\n", "a\nb", "a\rb", "a\r\nb", "\n"] {
        assert!(needs_paste_confirmation(text, false), "{text:?}");
    }
}

#[test]
fn bracketed_paste_never_needs_confirmation() {
    assert!(!needs_paste_confirmation("a\nb\nc", true));
}

#[test]
fn lines_are_counted_across_every_line_ending() {
    assert_eq!(paste_line_count("a"), 1);
    assert_eq!(paste_line_count("a\n"), 1);
    assert_eq!(paste_line_count("a\nb\n"), 2);
    assert_eq!(paste_line_count("a\r\nb\rc"), 3);
}

#[test]
fn the_preview_shows_the_first_five_lines_and_marks_the_rest() {
    let text = "1\n2\n3\n4\n5\n6\n7";

    assert_eq!(paste_preview(text), "1\n2\n3\n4\n5\n…");
    assert_eq!(paste_preview("1\n2"), "1\n2");
}

#[test]
fn a_long_line_is_clipped_with_an_ellipsis() {
    let preview = paste_preview(&"x".repeat(200));

    assert_eq!(preview.chars().count(), PREVIEW_LINE_CHARACTERS + 1);
    assert!(preview.ends_with('…'));
}

#[test]
fn control_characters_are_shown_escaped_but_tabs_stay() {
    assert_eq!(paste_preview("a\u{1b}[31mb"), "a^[[31mb");
    assert_eq!(paste_preview("bell\u{7}"), "bell^G");
    assert_eq!(paste_preview("del\u{7f}"), "del^?");
    assert_eq!(paste_preview("c1\u{85}"), "c1\\u{85}");
    assert_eq!(paste_preview("a\tb"), "a\tb");
}

#[test]
fn a_control_character_that_runs_the_line_needs_confirmation_without_any_newline() {
    for text in ["rm -rf ~\u{f}", "x\u{4}", "x\u{7f}", "x\u{1b}[200~"] {
        assert!(needs_paste_confirmation(text, false), "{text:?}");
        assert!(!needs_paste_confirmation(text, true), "{text:?}");
    }
    assert!(!needs_paste_confirmation("a\tb", false));
}

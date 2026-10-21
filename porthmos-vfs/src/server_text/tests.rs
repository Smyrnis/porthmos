use super::*;

#[test]
fn plain_text_is_returned_unchanged() {
    assert_eq!(clean_server_text("OpenSSH_9.6p1", 40).as_deref(), Some("OpenSSH_9.6p1"));
}

#[test]
fn whitespace_runs_are_collapsed_and_the_ends_trimmed() {
    assert_eq!(clean_server_text("  nginx \t 1.25  \r\n ", 40).as_deref(), Some("nginx 1.25"));
}

#[test]
fn control_and_bidi_characters_become_spaces() {
    assert_eq!(clean_server_text("a\u{1b}[2Jb\u{202e}c\u{7}d", 40).as_deref(), Some("a [2Jb c d"));
    assert_eq!(clean_server_text("x\u{2066}y\u{200f}z\u{61c}w", 40).as_deref(), Some("x y z w"));
}

#[test]
fn empty_or_only_blank_text_is_none() {
    for text in ["", "   ", "\u{1b}\u{7}\t\n", "\u{202e}"] {
        assert_eq!(clean_server_text(text, 40), None, "{text:?}");
    }
}

#[test]
fn long_text_is_cut_to_the_width_with_an_ellipsis() {
    let cleaned = clean_server_text(&"x".repeat(100), 40).unwrap();

    assert_eq!(cleaned.chars().count(), 40);
    assert!(cleaned.ends_with('\u{2026}'));
    assert_eq!(clean_server_text(&"x".repeat(40), 40).unwrap(), "x".repeat(40));
}

#[test]
fn wide_characters_are_cut_by_display_width() {
    let cleaned = clean_server_text(&"\u{65e5}".repeat(50), 40).unwrap();

    assert!(cleaned.ends_with('\u{2026}'));
    assert!(unicode_width::UnicodeWidthStr::width(cleaned.as_str()) <= 40);
}

#[test]
fn a_zero_width_budget_gives_none() {
    assert_eq!(clean_server_text("abc", 0), None);
}

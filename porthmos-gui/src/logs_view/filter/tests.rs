use porthmos_core::{LogLevel, LogLine};

use super::*;

fn stored(text: &str, level: LogLevel) -> StoredLine {
    StoredLine { seq: 0, line: LogLine { text: text.to_string(), level }, skipped: None }
}

fn marker() -> StoredLine {
    StoredLine { seq: 0, line: LogLine { text: String::new(), level: LogLevel::Unknown }, skipped: Some(5) }
}

#[test]
fn an_empty_needle_matches_everything() {
    assert!(text_matches("anything", ""));
    assert!(text_matches("", ""));
}

#[test]
fn ascii_matching_ignores_case() {
    assert!(text_matches("Connection Refused by peer", "refused"));
    assert!(text_matches("connection refused", "REFUSED"));
}

#[test]
fn a_missing_word_does_not_match() {
    assert!(!text_matches("abc", "abcd"));
    assert!(!text_matches("abcdef", "xyz"));
}

#[test]
fn a_non_ascii_needle_is_matched_on_the_lowercased_line() {
    assert!(text_matches("Σφάλμα ΣΎΝΔΕΣΗΣ", "σύνδεσης"));
    assert!(text_matches("ünï code", "ÜNÏ"));
}

#[test]
fn a_needle_longer_than_the_line_does_not_match() {
    assert!(!text_matches("ab", "abc"));
}

#[test]
fn a_new_filter_is_inactive_and_passes_everything() {
    let filter = LogFilter::default();

    assert!(!filter.is_active());
    for level in [LogLevel::Error, LogLevel::Warn, LogLevel::Info, LogLevel::Debug, LogLevel::Unknown] {
        assert!(filter.passes(&stored("x", level)));
    }
}

#[test]
fn a_text_makes_the_filter_active() {
    let filter = LogFilter { text: "a".to_string(), ..LogFilter::default() };

    assert!(filter.is_active());
}

#[test]
fn a_level_pill_makes_the_filter_active() {
    let filter = LogFilter { warn: true, ..LogFilter::default() };

    assert!(filter.is_active());
}

#[test]
fn toggle_flips_one_pill_and_ignores_unknown() {
    let mut filter = LogFilter::default();

    filter.toggle(LogLevel::Info);
    assert!(filter.level_on(LogLevel::Info));
    assert!(!filter.level_on(LogLevel::Error));

    filter.toggle(LogLevel::Info);
    assert!(!filter.is_active());

    filter.toggle(LogLevel::Unknown);
    assert!(!filter.is_active());
    assert!(!filter.level_on(LogLevel::Unknown));
}

#[test]
fn with_no_pill_every_level_passes() {
    let filter = LogFilter { text: "x".to_string(), ..LogFilter::default() };

    for level in [LogLevel::Error, LogLevel::Warn, LogLevel::Info, LogLevel::Debug, LogLevel::Unknown] {
        assert!(filter.passes(&stored("x", level)), "{level:?}");
    }
}

#[test]
fn with_the_error_pill_only_error_lines_pass() {
    let filter = LogFilter { error: true, ..LogFilter::default() };

    assert!(filter.passes(&stored("x", LogLevel::Error)));
    assert!(!filter.passes(&stored("x", LogLevel::Unknown)));
    assert!(!filter.passes(&stored("x", LogLevel::Warn)));
    assert!(!filter.passes(&stored("x", LogLevel::Info)));
    assert!(!filter.passes(&stored("x", LogLevel::Debug)));
}

#[test]
fn several_pills_pass_each_of_their_levels() {
    let filter = LogFilter { warn: true, debug: true, ..LogFilter::default() };

    assert!(filter.passes(&stored("x", LogLevel::Warn)));
    assert!(filter.passes(&stored("x", LogLevel::Debug)));
    assert!(!filter.passes(&stored("x", LogLevel::Error)));
    assert!(!filter.passes(&stored("x", LogLevel::Info)));
}

#[test]
fn a_skipped_marker_is_hidden_by_any_active_filter_and_shown_without_one() {
    assert!(LogFilter::default().passes(&marker()));
    assert!(!LogFilter { text: "x".to_string(), ..LogFilter::default() }.passes(&marker()));
    assert!(!LogFilter { info: true, ..LogFilter::default() }.passes(&marker()));
}

#[test]
fn a_whitespace_only_text_is_not_a_filter() {
    let filter = LogFilter { text: "  ".to_string(), ..LogFilter::default() };

    assert!(!filter.is_active());
    assert!(filter.passes(&stored("anything", LogLevel::Info)));
}

#[test]
fn spaces_inside_the_text_still_match() {
    assert!(text_matches("disk is full", "k is"));
}

#[test]
fn a_greek_final_sigma_does_not_break_matching() {
    assert!(text_matches("ΟΔΟΣΤΡΩΜΑ", "ΟΔΟΣ"));
    assert!(text_matches("η οδος", "ΟΔΟΣ"));
    assert!(text_matches("ΟΔΟΣ", "οδοσ"));
    assert!(text_matches("σφάλμα", "Σ"));
}

#[test]
fn text_and_level_must_both_pass() {
    let filter = LogFilter { text: "disk".to_string(), error: true, ..LogFilter::default() };

    assert!(filter.passes(&stored("Disk full", LogLevel::Error)));
    assert!(!filter.passes(&stored("Disk full", LogLevel::Info)));
    assert!(!filter.passes(&stored("network down", LogLevel::Error)));
}

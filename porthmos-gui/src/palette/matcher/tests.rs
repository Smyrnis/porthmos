use std::time::{Duration, Instant};

use super::*;

fn indices(query: &str, candidate: &str) -> Vec<usize> {
    fuzzy_match(query, candidate).unwrap_or_else(|| panic!("{query:?} should match {candidate:?}")).indices
}

fn score(query: &str, candidate: &str) -> i32 {
    fuzzy_match(query, candidate).unwrap_or_else(|| panic!("{query:?} should match {candidate:?}")).score
}

#[test]
fn an_empty_query_matches_everything_with_no_indices() {
    for candidate in ["", "copy", "\u{65e5}\u{672c}"] {
        let found = fuzzy_match("", candidate).unwrap();
        assert_eq!((found.score, found.indices), (0, Vec::new()), "{candidate:?}");
    }
}

#[test]
fn a_query_that_is_not_a_subsequence_does_not_match() {
    assert!(fuzzy_match("xyz", "copy").is_none());
    assert!(fuzzy_match("pc", "copy").is_none());
    assert!(fuzzy_match("a", "").is_none());
}

#[test]
fn matching_ignores_case_both_ways() {
    assert_eq!(indices("CON", "connect"), [0, 1, 2]);
    assert_eq!(indices("con", "CONNECT"), [0, 1, 2]);
}

#[test]
fn the_indices_are_ascending_character_positions_that_spell_the_query() {
    assert_eq!(indices("nc", "New connection"), [0, 4]);
    let found = indices("ect", "Disconnect now");
    assert!(found.windows(2).all(|pair| pair[0] < pair[1]));
    let letters: String = found.iter().map(|index| "Disconnect now".chars().nth(*index).unwrap()).collect();
    assert_eq!(letters.to_lowercase(), "ect");
}

#[test]
fn indices_count_characters_not_bytes() {
    assert_eq!(indices("\u{8a9e}\u{30d5}", "\u{65e5}\u{672c}\u{8a9e}\u{30d5}\u{30a1}\u{30a4}\u{30eb}"), [2, 3]);
}

#[test]
fn a_query_longer_than_the_candidate_does_not_match() {
    assert!(fuzzy_match("copyx", "copy").is_none());
}

#[test]
fn consecutive_matches_beat_scattered_ones() {
    assert!(score("conn", "Connect") > score("conn", "Cut one nine"));
}

#[test]
fn a_word_start_beats_the_middle_of_a_word() {
    assert!(score("nc", "New connection") > score("nc", "Unconnected"));
}

#[test]
fn an_earlier_match_beats_a_later_one() {
    assert!(score("ab", "xab") > score("ab", "xxxxab"));
    assert!(score("co", "Copy") > score("co", "Disconnect"));
}

#[test]
fn a_shorter_candidate_wins_an_otherwise_equal_match() {
    assert!(score("cop", "Copy") > score("cop", "Copy to the other panel"));
}

#[test]
fn camel_case_steps_count_as_word_starts() {
    assert!(score("fb", "FooBar") > score("fb", "fxxbar"));
}

#[test]
fn separators_start_words() {
    let plain = score("b", "foobar");
    for candidate in ["foo-bar", "foo/bar", "foo_bar", "foo.bar", "foo bar", "foo:bar"] {
        assert_eq!(indices("b", candidate), [4], "{candidate}");
        assert!(score("b", candidate) > plain, "{candidate}");
    }
}

#[test]
fn repeated_query_characters_use_distinct_positions() {
    let found = indices("aa", "banana");

    assert_eq!(found.len(), 2);
    assert!(found[0] < found[1]);
    assert!(found.iter().all(|index| "banana".chars().nth(*index) == Some('a')));
}

#[test]
fn the_best_alignment_is_chosen_not_the_first_one_found() {
    assert_eq!(indices("ab", "a xxxx ab"), [7, 8]);
}

#[test]
fn ten_thousand_candidates_match_within_the_budget() {
    let candidates: Vec<String> = (0..10_000).map(|index| format!("dir{index}/file_{index}.txt")).collect();

    let start = Instant::now();
    let matched = candidates.iter().filter(|candidate| fuzzy_match("fl9", candidate).is_some()).count();

    assert!(start.elapsed() < Duration::from_secs(1), "{:?}", start.elapsed());
    assert!(matched > 0);
}

#[test]
fn a_five_hundred_character_candidate_with_a_hundred_character_query_is_fast_and_does_not_panic() {
    let candidate = "ab".repeat(250);
    let query = "ab".repeat(50);

    let start = Instant::now();
    let found = fuzzy_match(&query, &candidate).unwrap();

    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(found.indices.len(), 100);
}

#[test]
fn a_hundred_thousand_character_query_does_not_panic() {
    let query = "a".repeat(100_000);

    assert!(fuzzy_match(&query, "banana").is_none());
    assert!(fuzzy_match(&query, &"a".repeat(100_000)).is_some_and(|found| found.indices.len() == 100_000));
}

#[test]
fn unusual_characters_do_not_panic() {
    for candidate in ["\u{1b}[2J", "a\u{202e}b", "\u{0}", "\u{1f600}\u{1f600}", "\u{130}stanbul"] {
        let _ = fuzzy_match("a", candidate);
        let _ = fuzzy_match("\u{1f600}", candidate);
        let _ = fuzzy_match("i", candidate);
    }
}

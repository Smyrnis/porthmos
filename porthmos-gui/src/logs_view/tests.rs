use porthmos_core::{LogLevel, LogLine, LogSource};

use super::*;

fn line(text: &str) -> LogLine {
    LogLine { text: text.to_string(), level: LogLevel::Info }
}

fn lines(texts: &[&str]) -> Vec<LogLine> {
    texts.iter().map(|text| line(text)).collect()
}

fn texts(state: &LogsState) -> Vec<String> {
    state.lines.iter().map(|stored| stored.line.text.clone()).collect()
}

fn shown_texts(state: &LogsState) -> Vec<String> {
    state.shown().map(|stored| stored.line.text.clone()).collect()
}

#[test]
fn a_file_source_needs_a_path() {
    let mut state = LogsState::default();
    assert_eq!(state.source(), None);

    state.path = "/var/log/syslog".to_string();

    assert_eq!(state.source(), Some(LogSource::File("/var/log/syslog".to_string())));
}

#[test]
fn a_blank_path_is_no_source() {
    let state = LogsState { path: "   ".to_string(), ..LogsState::default() };

    assert_eq!(state.source(), None);
}

#[test]
fn the_journal_source_takes_an_optional_valid_unit() {
    let mut state = LogsState { kind: SourceKind::Journal, ..LogsState::default() };
    assert_eq!(state.source(), Some(LogSource::Journal(None)));

    state.unit = "nginx.service".to_string();

    assert_eq!(state.source(), Some(LogSource::Journal(UnitName::new("nginx.service"))));
}

#[test]
fn a_hostile_unit_is_no_source() {
    for unit in ["nginx.service; rm -rf /", "-x.service", "$(id).service", "nginx"] {
        let state = LogsState { kind: SourceKind::Journal, unit: unit.to_string(), ..LogsState::default() };

        assert_eq!(state.source(), None, "{unit}");
    }
}

#[test]
fn the_unit_is_ignored_for_a_file_source() {
    let state = LogsState { path: "/a".to_string(), unit: "bad unit".to_string(), ..LogsState::default() };

    assert_eq!(state.source(), Some(LogSource::File("/a".to_string())));
}

#[test]
fn lines_append_in_order_with_growing_sequence_numbers() {
    let mut state = LogsState::default();

    state.append(lines(&["a", "b"]), 0, 100);
    state.append(lines(&["c"]), 0, 100);

    assert_eq!(texts(&state), ["a", "b", "c"]);
    let seqs: Vec<u64> = state.lines.iter().map(|stored| stored.seq).collect();
    assert_eq!(seqs, [0, 1, 2]);
    assert_eq!(state.next_seq, 3);
}

#[test]
fn the_oldest_lines_are_dropped_past_the_cap() {
    let mut state = LogsState::default();

    state.append(lines(&["a", "b", "c", "d"]), 0, 3);

    assert_eq!(texts(&state), ["b", "c", "d"]);
    assert_eq!(state.lines.front().map(|stored| stored.seq), Some(1));
}

#[test]
fn a_smaller_cap_trims_on_the_next_append() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b", "c", "d"]), 0, 100);

    state.append(lines(&["e"]), 0, 2);

    assert_eq!(texts(&state), ["d", "e"]);
}

#[test]
fn skipped_lines_add_one_counted_marker_before_the_new_lines() {
    let mut state = LogsState::default();

    state.append(lines(&["a"]), 7, 100);

    let markers: Vec<Option<u64>> = state.lines.iter().map(|stored| stored.skipped).collect();
    assert_eq!(markers, [Some(7), None]);
    let marker = &state.lines[0].line;
    assert_eq!(marker.text, "");
    assert_eq!(marker.level, LogLevel::Unknown);
    assert_eq!(state.lines[1].line.text, "a");
}

#[test]
fn each_skip_gets_its_own_marker_and_zero_adds_no_marker() {
    let mut state = LogsState::default();

    state.append(lines(&["a"]), 0, 100);
    state.append(lines(&["b"]), 3, 100);
    state.append(lines(&["c"]), 4, 100);

    let markers: Vec<u64> = state.lines.iter().filter_map(|stored| stored.skipped).collect();
    assert_eq!(markers, [3, 4]);
    assert_eq!(state.lines.len(), 5);
}

#[test]
fn pausing_freezes_the_shown_range_and_counts_new_lines() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b"]), 0, 100);

    state.pause_toggled(100);
    state.append(lines(&["c", "d", "e"]), 0, 100);

    assert!(state.paused());
    assert_eq!(shown_texts(&state), ["a", "b"]);
    assert_eq!(state.unseen_since_pause(), 3);
}

#[test]
fn resuming_shows_everything_again() {
    let mut state = LogsState::default();
    state.append(lines(&["a"]), 0, 100);
    state.pause_toggled(100);
    state.append(lines(&["b"]), 0, 100);

    state.pause_toggled(100);

    assert!(!state.paused());
    assert_eq!(shown_texts(&state), ["a", "b"]);
    assert_eq!(state.unseen_since_pause(), 0);
}

fn marker_counts(state: &LogsState) -> Vec<Option<u64>> {
    state.lines.iter().map(|stored| stored.skipped).collect()
}

#[test]
fn a_pause_at_the_cap_keeps_the_shown_lines_and_counts_what_it_could_not_keep() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b", "c"]), 0, 3);
    state.pause_toggled(3);

    state.append(lines(&["d", "e", "f"]), 0, 3);
    state.append(lines(&["g", "h"]), 4, 3);

    assert_eq!(shown_texts(&state), ["a", "b", "c"]);
    assert_eq!(texts(&state), ["a", "b", "c"]);
    assert_eq!(state.unseen_since_pause(), 9);

    state.pause_toggled(3);
    assert!(!state.paused());
    assert_eq!(texts(&state), ["b", "c", ""]);
    assert_eq!(marker_counts(&state), [None, None, Some(9)]);
    assert_eq!(state.unseen_since_pause(), 0);

    state.append(lines(&["new"]), 0, 3);
    assert_eq!(texts(&state), ["c", "", "new"]);
}

#[test]
fn a_pause_below_the_cap_keeps_lines_until_the_cap_then_counts_the_rest() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b"]), 0, 4);
    state.pause_toggled(4);

    state.append(lines(&["c", "d", "e"]), 0, 4);

    assert_eq!(shown_texts(&state), ["a", "b"]);
    assert_eq!(state.unseen_since_pause(), 3);
    state.pause_toggled(4);
    assert_eq!(texts(&state), ["b", "c", "d", ""]);
    assert_eq!(marker_counts(&state), [None, None, None, Some(1)]);
}

#[test]
fn resuming_without_anything_held_back_adds_no_marker() {
    let mut state = LogsState::default();
    state.append(lines(&["a"]), 0, 3);
    state.pause_toggled(3);
    state.append(lines(&["b"]), 0, 3);

    state.pause_toggled(3);

    assert_eq!(marker_counts(&state), [None, None]);
}

#[test]
fn clearing_while_paused_forgets_the_held_back_count() {
    let mut state = LogsState::default();
    state.append(lines(&["a"]), 0, 1);
    state.pause_toggled(1);
    state.append(lines(&["b", "c"]), 0, 1);
    assert_eq!(state.unseen_since_pause(), 2);

    state.clear_buffer();

    assert_eq!(state.unseen_since_pause(), 0);
    state.pause_toggled(1);
    assert!(state.lines.is_empty());
}

#[test]
fn starting_fresh_forgets_the_held_back_count() {
    let mut state = LogsState::default();
    state.append(lines(&["a"]), 0, 1);
    state.pause_toggled(1);
    state.append(lines(&["b"]), 0, 1);

    state.start_fresh();
    state.pause_toggled(1);
    state.pause_toggled(1);

    assert!(state.lines.is_empty());
}

#[test]
fn trimming_drops_the_oldest_lines_at_once() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b", "c", "d"]), 0, 100);

    state.trim(2);

    assert_eq!(texts(&state), ["c", "d"]);
    state.trim(5);
    assert_eq!(texts(&state), ["c", "d"]);
}

#[test]
fn the_shown_length_stays_right_after_old_lines_are_trimmed_during_a_pause() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b", "c", "d"]), 0, 100);
    state.pause_toggled(100);
    state.append(lines(&["e", "f"]), 0, 100);

    state.trim(3);

    assert_eq!(shown_texts(&state), ["d"]);
    assert_eq!(state.shown_len(), 1);
    assert_eq!(state.unseen_since_pause(), 2);
    state.trim(1);
    assert_eq!(state.shown_len(), 0);
    assert_eq!(state.unseen_since_pause(), 1);
}

#[test]
fn clearing_the_buffer_keeps_sequence_numbers_growing() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b"]), 4, 100);

    state.clear_buffer();
    state.append(lines(&["c"]), 0, 100);

    assert_eq!(texts(&state), ["c"]);
    assert_eq!(state.lines[0].seq, 3);
}

#[test]
fn clearing_while_paused_keeps_the_pause_and_hides_what_follows() {
    let mut state = LogsState::default();
    state.append(lines(&["a"]), 0, 100);
    state.pause_toggled(100);

    state.clear_buffer();
    state.append(lines(&["b"]), 0, 100);

    assert!(state.paused());
    assert!(shown_texts(&state).is_empty());
    assert_eq!(state.unseen_since_pause(), 1);
}

#[test]
fn starting_fresh_empties_the_buffer_drops_a_pause_and_follows_again() {
    let mut state = LogsState::default();
    state.append(lines(&["a", "b"]), 0, 100);
    state.pause_toggled(100);
    state.append(lines(&["c"]), 0, 100);
    state.scrolled(40.0, 100.0, 900.0);

    state.start_fresh();

    assert!(state.lines.is_empty());
    assert!(!state.paused());
    assert!(state.follow);
    assert_eq!(state.scroll, 0.0);
    state.append(lines(&["d"]), 0, 100);
    assert_eq!(shown_texts(&state), ["d"]);
    assert_eq!(state.unseen_since_pause(), 0);
}

#[test]
fn clearing_resets_the_scroll_position_and_keeps_the_follow_choice() {
    let mut state = LogsState::default();
    state.append(lines(&["a"]), 0, 100);
    state.scrolled(10.0, 100.0, 900.0);

    state.clear_buffer();

    assert!(!state.follow);
    assert_eq!(state.scroll, 0.0);
}

#[test]
fn the_shown_length_matches_the_shown_lines_with_and_without_a_pause() {
    let mut state = LogsState::default();
    assert_eq!(state.shown_len(), 0);
    state.append(lines(&["a", "b", "c"]), 0, 100);
    assert_eq!(state.shown_len(), state.shown().count());

    state.pause_toggled(100);
    state.append(lines(&["d", "e"]), 0, 100);

    assert_eq!(state.shown_len(), 3);
    assert_eq!(state.shown_len(), state.shown().count());
}

#[test]
fn scrolling_up_turns_follow_off_and_reaching_the_end_turns_it_on() {
    let mut state = LogsState::default();
    assert!(state.follow);

    state.scrolled(500.0, 200.0, 1000.0);
    assert!(!state.follow);
    assert_eq!(state.scroll, 500.0);

    state.scrolled(800.0, 200.0, 1000.0);
    assert!(state.follow);
}

#[test]
fn follow_counts_a_pixel_short_of_the_end_as_the_end() {
    assert!(follow_after_scroll(799.0, 200.0, 1000.0));
    assert!(follow_after_scroll(800.0, 200.0, 1000.0));
    assert!(!follow_after_scroll(700.0, 200.0, 1000.0));
}

#[test]
fn content_shorter_than_the_viewport_is_always_at_the_end() {
    assert!(follow_after_scroll(0.0, 400.0, 100.0));
    assert!(follow_after_scroll(0.0, 400.0, 0.0));
}

#[test]
fn a_snap_is_wanted_once_after_lines_arrive_while_following() {
    let mut state = LogsState::default();
    assert!(!state.take_snap());

    state.append(lines(&["a"]), 0, 100);

    assert!(state.take_snap());
    assert!(!state.take_snap());
}

#[test]
fn no_snap_is_wanted_while_not_following_until_follow_is_turned_back_on() {
    let mut state = LogsState::default();
    state.scrolled(0.0, 100.0, 1000.0);
    assert!(!state.follow);

    state.append(lines(&["a"]), 0, 100);
    assert!(!state.take_snap());

    state.follow_toggled(true);
    assert!(state.follow);
    assert!(state.take_snap());

    state.follow_toggled(false);
    assert!(!state.follow);
    assert!(!state.take_snap());
}

#[test]
fn a_view_counts_as_appearing_only_on_the_change_from_hidden_to_shown() {
    let mut state = LogsState::default();

    assert!(!state.appeared(false));
    assert!(state.appeared(true));
    assert!(!state.appeared(true));
    assert!(!state.appeared(false));
    assert!(state.appeared(true));
}

fn budgeted(bytes: usize) -> LogsState {
    LogsState { byte_budget: bytes, ..LogsState::default() }
}

fn stored_bytes(state: &LogsState) -> usize {
    state.lines.iter().map(|stored| stored.line.text.len()).sum()
}

#[test]
fn long_lines_are_evicted_by_bytes_before_the_line_cap() {
    let mut state = budgeted(10);

    state.append(lines(&["aaaa", "bbbb", "cccc"]), 0, 100);

    assert_eq!(texts(&state), ["bbbb", "cccc"]);
    assert_eq!(state.bytes, 8);
}

#[test]
fn the_newest_line_is_kept_even_past_the_byte_budget() {
    let mut state = budgeted(3);

    state.append(lines(&["aa", "bbbbbb"]), 0, 100);

    assert_eq!(texts(&state), ["bbbbbb"]);
    assert_eq!(state.bytes, 6);
}

#[test]
fn short_lines_still_keep_the_line_cap() {
    let mut state = budgeted(1000);

    state.append(lines(&["a", "b", "c", "d"]), 0, 3);

    assert_eq!(texts(&state), ["b", "c", "d"]);
    assert_eq!(state.bytes, 3);
}

#[test]
fn the_byte_total_stays_exact_across_eviction_trim_clear_and_a_fresh_start() {
    let mut state = budgeted(1000);
    state.append(lines(&["one", "two", "three"]), 2, 100);
    assert_eq!(state.bytes, stored_bytes(&state));

    state.trim(2);
    assert_eq!(state.bytes, stored_bytes(&state));
    assert_eq!(state.bytes, 8);

    state.clear_buffer();
    assert_eq!(state.bytes, 0);

    state.append(lines(&["four"]), 0, 100);
    state.start_fresh();
    assert_eq!(state.bytes, 0);
}

#[test]
fn a_pause_holds_when_the_byte_budget_is_the_limit_reached() {
    let mut state = budgeted(8);
    state.append(lines(&["aaaa", "bbbb"]), 0, 100);
    state.pause_toggled(100);

    state.append(lines(&["cc", "dd"]), 0, 100);

    assert_eq!(shown_texts(&state), ["aaaa", "bbbb"]);
    assert_eq!(texts(&state), ["aaaa", "bbbb"]);
    assert_eq!(state.unseen_since_pause(), 2);
    assert_eq!(state.bytes, 8);

    state.pause_toggled(100);
    assert_eq!(marker_counts(&state), [None, None, Some(2)]);
    assert_eq!(state.bytes, stored_bytes(&state));
}

#[test]
fn the_default_byte_budget_is_32_mib() {
    assert_eq!(MAX_BUFFER_BYTES, 32 * 1024 * 1024);
    assert_eq!(LogsState::default().byte_budget, MAX_BUFFER_BYTES);
}

#[test]
fn the_name_of_a_file_tab_is_its_last_path_component() {
    let name = |path: &str| LogsState { path: path.to_string(), ..LogsState::default() }.name();

    assert_eq!(name("/var/log/syslog"), TabName::File("syslog".to_string()));
    assert_eq!(name("  /var/log/syslog  "), TabName::File("syslog".to_string()));
    assert_eq!(name("/var/log/"), TabName::File("log".to_string()));
    assert_eq!(name("/"), TabName::File("/".to_string()));
    assert_eq!(name("   "), TabName::Empty);
}

#[test]
fn the_name_of_a_journal_tab_is_its_unit_or_the_whole_journal() {
    let name =
        |unit: &str| LogsState { kind: SourceKind::Journal, unit: unit.to_string(), ..LogsState::default() }.name();

    assert_eq!(name("nginx.service"), TabName::Journal(Some("nginx.service".to_string())));
    assert_eq!(name("  "), TabName::Journal(None));
}

#[test]
fn only_an_untouched_tab_is_pristine() {
    assert!(LogsState::default().is_pristine());
    assert!(!LogsState { path: "/a".to_string(), ..LogsState::default() }.is_pristine());
    assert!(!LogsState { unit: "x.service".to_string(), ..LogsState::default() }.is_pristine());
    assert!(!LogsState { status: LogStatus::Starting, ..LogsState::default() }.is_pristine());
    assert!(!LogsState { running: Some((1, 1)), ..LogsState::default() }.is_pristine());
    let mut with_lines = LogsState::default();
    with_lines.append(lines(&["a"]), 0, 10);
    assert!(!with_lines.is_pristine());
}

use chrono::NaiveDate;
use porthmos_core::{LogLevel, LogLine};

use super::*;
use crate::logs_view::{LogsState, SourceKind};

fn line(text: &str, level: LogLevel) -> LogLine {
    LogLine { text: text.to_string(), level }
}

fn state_with(texts: &[&str]) -> LogsState {
    let mut state = LogsState::default();
    state.append(texts.iter().map(|text| line(text, LogLevel::Info)).collect(), 0, 1000);
    state
}

fn marker_text(count: u64) -> String {
    format!("<{count} skipped>")
}

fn date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 28).unwrap()
}

#[test]
fn lines_are_joined_with_newlines_and_end_with_one() {
    let state = state_with(&["one", "two", "three"]);

    assert_eq!(save_text(&state, marker_text), "one\ntwo\nthree\n");
}

#[test]
fn an_empty_view_saves_an_empty_string() {
    assert_eq!(save_text(&LogsState::default(), marker_text), "");
}

#[test]
fn a_filter_saves_only_matching_lines_in_order() {
    let mut state = state_with(&["disk a", "net b", "Disk c"]);
    state.set_filter_text("disk".to_string());

    assert_eq!(save_text(&state, marker_text), "disk a\nDisk c\n");
}

#[test]
fn a_skipped_marker_is_written_as_its_text() {
    let mut state = LogsState::default();
    state.append(vec![line("after", LogLevel::Info)], 7, 1000);

    assert_eq!(save_text(&state, marker_text), "<7 skipped>\nafter\n");
}

#[test]
fn while_paused_only_lines_before_the_pause_are_saved() {
    let mut state = state_with(&["before"]);
    state.pause_toggled(1000);
    state.append(vec![line("after", LogLevel::Info)], 0, 1000);

    assert_eq!(save_text(&state, marker_text), "before\n");
}

#[test]
fn a_file_uses_its_last_path_part_and_the_date() {
    let state = LogsState { path: "/var/log/syslog".to_string(), ..LogsState::default() };

    assert_eq!(default_file_name(&state, date()), "syslog-2026-11-28.log");
}

#[test]
fn a_unit_uses_its_name() {
    let state = LogsState { kind: SourceKind::Journal, unit: "nginx.service".to_string(), ..LogsState::default() };

    assert_eq!(default_file_name(&state, date()), "nginx.service-2026-11-28.log");
}

#[test]
fn the_whole_journal_uses_the_word_journal() {
    let state = LogsState { kind: SourceKind::Journal, ..LogsState::default() };

    assert_eq!(default_file_name(&state, date()), "journal-2026-11-28.log");
}

#[test]
fn an_odd_source_is_made_safe() {
    let state = LogsState { path: "/tmp/we ird\u{1b}[31m:na*me?".to_string(), ..LogsState::default() };

    let name = default_file_name(&state, date());

    assert_eq!(name, "we_ird__31m_na_me_-2026-11-28.log");
    assert!(!name.contains('/') && !name.chars().any(char::is_control));
}

#[test]
fn an_empty_source_becomes_log() {
    assert_eq!(default_file_name(&LogsState::default(), date()), "log-2026-11-28.log");
}

#[test]
fn greek_letters_are_kept() {
    let state = LogsState { path: "/var/log/σφάλμα".to_string(), ..LogsState::default() };

    assert_eq!(default_file_name(&state, date()), "σφάλμα-2026-11-28.log");
}

#[test]
fn a_very_long_source_is_cut_to_the_cap_on_a_character_boundary() {
    let state = LogsState { path: format!("/var/log/{}", "σ".repeat(300)), ..LogsState::default() };

    let name = default_file_name(&state, date());

    assert!(name.len() <= MAX_SOURCE_BYTES + "-2026-11-28.log".len(), "{}", name.len());
    assert!(name.ends_with("-2026-11-28.log"));
    assert!(name.starts_with('σ'));
}

#[test]
fn the_cap_is_two_hundred_bytes() {
    assert_eq!(MAX_SOURCE_BYTES, 200);
}

#[test]
fn windows_device_names_get_a_prefix() {
    for source in ["con", "CON", "nul.txt", "Com1", "lpt9.log", "aux", "PRN"] {
        let state = LogsState { path: format!("/x/{source}"), ..LogsState::default() };

        assert!(default_file_name(&state, date()).starts_with('_'), "{source}");
    }
}

#[test]
fn names_that_only_start_like_a_device_are_left_alone() {
    for source in ["console", "com10", "auxiliary.log", "lpt0", "nullable"] {
        let state = LogsState { path: format!("/x/{source}"), ..LogsState::default() };

        assert!(!default_file_name(&state, date()).starts_with('_'), "{source}");
    }
}

#[tokio::test]
async fn writing_creates_the_file_with_the_text() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.log");

    write_lines(path.clone(), "a\nb\n".to_string()).await.unwrap();

    assert_eq!(std::fs::read_to_string(path).unwrap(), "a\nb\n");
}

#[tokio::test]
async fn writing_into_a_missing_folder_reports_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing").join("out.log");

    let error = write_lines(path, "a\n".to_string()).await.unwrap_err();

    assert!(!error.is_empty());
}

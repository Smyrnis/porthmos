use chrono::{TimeZone, Utc};
use porthmos_core::history::{HistoryEntry, HistoryResult};
use porthmos_core::transfer::Direction;

use super::{HistoryState, details, printable, row_text};

fn entry(label: &str, connection: &str, result: HistoryResult) -> HistoryEntry {
    HistoryEntry {
        finished_at: Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap(),
        connection: connection.to_string(),
        direction: Direction::Upload,
        label: label.to_string(),
        local_path: "/l".to_string(),
        remote_path: "/r".to_string(),
        files_done: 2,
        files_total: 3,
        bytes: 2048,
        result,
        failed_count: 0,
        failed_files: Vec::new(),
    }
}

fn done(label: &str, connection: &str) -> HistoryEntry {
    entry(label, connection, HistoryResult::Done)
}

fn sample() -> Vec<HistoryEntry> {
    vec![done("photos", "alpha"), done("music", "beta"), done("notes", "alpha")]
}

#[test]
fn a_new_state_selects_the_first_entry() {
    let state = HistoryState::new(sample());

    assert_eq!(state.selected().unwrap().label, "photos");
    assert_eq!(state.total(), 3);
}

#[test]
fn visible_applies_the_core_filter() {
    let mut state = HistoryState::new(sample());

    state.set_filter("alpha".to_string());

    let labels: Vec<_> = state.visible().iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(labels, ["photos", "notes"]);
    assert_eq!(state.filter(), "alpha");
}

#[test]
fn selection_stays_on_a_visible_row_when_the_filter_narrows() {
    let mut state = HistoryState::new(sample());
    state.select(2);

    state.set_filter("beta".to_string());

    assert_eq!(state.selected().unwrap().label, "music");
    state.set_filter("zzz".to_string());
    assert!(state.selected().is_none());
}

#[test]
fn move_selection_clamps_at_both_ends() {
    let mut state = HistoryState::new(sample());

    state.move_selection(false);
    assert_eq!(state.selected_row(), Some(0));
    for _ in 0..5 {
        state.move_selection(true);
    }
    assert_eq!(state.selected_row(), Some(2));
}

#[test]
fn replace_keeps_the_selection_clamped_and_the_filter() {
    let mut state = HistoryState::new(sample());
    state.select(2);
    state.set_filter("alpha".to_string());

    state.replace(vec![done("photos", "alpha")]);

    assert_eq!(state.filter(), "alpha");
    assert_eq!(state.selected_row(), Some(0));
    state.replace(Vec::new());
    assert!(state.selected().is_none());
}

#[test]
fn select_ignores_a_row_past_the_end() {
    let mut state = HistoryState::new(sample());

    state.select(9);

    assert_eq!(state.selected_row(), Some(0));
}

#[test]
fn a_thousand_entries_filter_to_the_matching_ones() {
    let entries =
        (0..1000).map(|index| done(&format!("job{index}"), if index % 10 == 0 { "alpha" } else { "beta" })).collect();
    let mut state = HistoryState::new(entries);

    state.set_filter("alpha".to_string());

    assert_eq!(state.visible().len(), 100);
    assert_eq!(state.total(), 1000);
}

#[test]
fn row_text_shows_result_and_counts() {
    let (title, summary) = row_text(&done("photos", "alpha"));

    assert_eq!(title, "photos");
    assert_eq!(summary, "alpha · Upload · done · 2/3 files · 2.0K");
}

#[test]
fn row_text_stays_on_one_line() {
    let (title, summary) = row_text(&done("a\nb\tc", "al\npha"));

    assert_eq!(title, "a?b?c");
    assert!(!summary.contains('\n'));
}

#[test]
fn printable_replaces_control_characters() {
    assert_eq!(printable("ok\u{1b}[31m\r\nx"), "ok?[31m??x");
    assert_eq!(printable("plain name.txt"), "plain name.txt");
}

#[test]
fn details_for_a_done_transfer_have_no_log_hint() {
    let (title, body) = details(&done("photos", "alpha"), &Utc);

    assert_eq!(title, "Transfer: photos");
    assert_eq!(
        body,
        "Finished:   2026-10-01 12:00:00\nConnection: alpha (Upload)\nResult:     done, 2 of 3 files, 2.0K\nLocal:      /l\nRemote:     /r"
    );
}

#[test]
fn details_for_a_failed_transfer_list_files_and_point_at_the_log() {
    let mut failed = entry("photos", "alpha", HistoryResult::PartlyFailed { failed: 2 });
    failed.failed_files = vec!["a.txt".to_string(), "b.txt".to_string()];
    failed.failed_count = 2;
    failed.direction = Direction::Download;

    let (_, body) = details(&failed, &Utc);

    assert!(body.contains("Connection: alpha (Download)"));
    assert!(body.contains("Result:     partly failed (2 failed), 2 of 3 files, 2.0K"));
    assert!(body.contains("Failed files:\n- a.txt\n- b.txt"));
    assert!(body.ends_with("The reasons are in porthmos.log."));
}

#[test]
fn details_cap_failed_files_at_ten_with_a_remainder() {
    let mut failed = entry("photos", "alpha", HistoryResult::Failed);
    failed.failed_files = (0..12).map(|index| format!("f{index}")).collect();
    failed.failed_count = 15;

    let (_, body) = details(&failed, &Utc);

    assert!(body.contains("- f9"));
    assert!(!body.contains("- f10"));
    assert!(body.contains("and 5 more…"));
}

#[test]
fn details_skip_empty_paths() {
    let mut local_only = done("photos", "alpha");
    local_only.remote_path.clear();

    let (_, body) = details(&local_only, &Utc);

    assert!(body.contains("Local:      /l"));
    assert!(!body.contains("Remote:"));
}

#[test]
fn details_make_hostile_names_printable() {
    let hostile = done("a\u{1b}[2Jb", "al\npha");

    let (title, body) = details(&hostile, &Utc);

    assert_eq!(title, "Transfer: a?[2Jb");
    assert!(!body.contains('\u{1b}'));
    assert!(body.contains("Connection: al?pha (Upload)"));
}

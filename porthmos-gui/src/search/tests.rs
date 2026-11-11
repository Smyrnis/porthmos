use porthmos_core::{Entry, Location, VfsPath};

use super::{PreviewSync, SearchPreview, SearchState, SearchStatus, result_parent};

fn root() -> VfsPath {
    VfsPath::new("/data")
}

fn state() -> SearchState {
    SearchState::new(Location::Local, root())
}

fn hit(path: &str) -> Entry {
    let path = VfsPath::new(path);
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    Entry { name, path, ..Entry::default() }
}

fn searching() -> SearchState {
    let mut state = state();
    state.set_pattern("a".to_string());
    state
}

#[test]
fn a_blank_pattern_sends_no_request_and_clears_results() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    assert!(state.set_pattern("   ".to_string()).is_none());
    assert!(state.results().is_empty());
    assert_eq!(state.status(), &SearchStatus::Idle);
    assert!(state.selected().is_none());
}

#[test]
fn a_pattern_is_wrapped_in_wildcards() {
    let mut state = state();
    let request = state.set_pattern("abc".to_string()).unwrap();
    assert_eq!(request.pattern, "*abc*");
    assert_eq!(request.root, root());
    assert_eq!(request.location, Location::Local);
    assert_eq!(state.pattern(), "abc");
    assert_eq!(state.status(), &SearchStatus::Searching);
}

#[test]
fn a_new_pattern_clears_the_previous_results_and_selection() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.found(hit("/data/b.txt"));
    state.set_pattern("ab".to_string());
    assert!(state.results().is_empty());
    assert!(state.selected().is_none());
    assert_eq!(state.status(), &SearchStatus::Searching);
}

#[test]
fn found_entries_accumulate_and_the_first_is_selected_on_arrival() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.found(hit("/data/b.txt"));
    assert_eq!(state.results().len(), 2);
    assert_eq!(state.selected().unwrap().name, "a.txt");
}

#[test]
fn finished_marks_done_and_truncated_is_remembered() {
    let mut state = searching();
    state.finished(true, false);
    assert_eq!(state.status(), &SearchStatus::Done { truncated: true, incomplete: false });
}

#[test]
fn failed_keeps_results_and_shows_the_message() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.failed("permission denied".to_string());
    assert_eq!(state.results().len(), 1);
    assert_eq!(state.status_text(), "Search failed: permission denied");
}

#[test]
fn move_selection_clamps_at_both_ends() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.found(hit("/data/b.txt"));
    state.move_selection(false);
    assert_eq!(state.selected().unwrap().name, "a.txt");
    state.move_selection(true);
    state.move_selection(true);
    assert_eq!(state.selected().unwrap().name, "b.txt");
}

#[test]
fn select_ignores_a_row_past_the_end() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.select(5);
    assert_eq!(state.selected().unwrap().name, "a.txt");
}

#[test]
fn status_text_for_every_status() {
    let mut state = state();
    assert_eq!(state.status_text(), "Type a name to search under /data");
    state.set_pattern("a".to_string());
    assert_eq!(state.status_text(), "Searching…");
    state.found(hit("/data/a.txt"));
    assert_eq!(state.status_text(), "Searching… 1 found");
    state.finished(false, false);
    assert_eq!(state.status_text(), "1 found");
    state.finished(true, false);
    assert_eq!(state.status_text(), "1 found");
}

#[test]
fn a_finished_search_with_nothing_says_so() {
    let mut state = searching();
    state.finished(false, false);
    assert_eq!(state.status_text(), "Nothing found");
}

#[test]
fn a_truncated_search_asks_to_narrow_it() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.found(hit("/data/b.txt"));
    state.finished(true, false);
    assert_eq!(state.status_text(), "Showing the first 2 matches; narrow the search");
}

#[test]
fn result_parent_falls_back_to_the_entry_path() {
    assert_eq!(result_parent(&hit("/data/x/a.txt")), VfsPath::new("/data/x"));
    let rootless = Entry { path: VfsPath::default(), ..Entry::default() };
    assert_eq!(result_parent(&rootless), VfsPath::default());
}

#[test]
fn late_results_after_a_blank_pattern_are_dropped() {
    let mut state = searching();
    state.set_pattern(String::new());
    state.found(hit("/data/late.txt"));
    state.finished(false, false);
    state.failed("late".to_string());
    assert!(state.results().is_empty());
    assert_eq!(state.status(), &SearchStatus::Idle);
}

#[test]
fn the_shown_parent_is_relative_to_the_search_root() {
    let state = state();

    assert_eq!(state.shown_parent(&hit("/data/a/b/x.txt")), "a/b");
}

#[test]
fn a_result_directly_in_the_root_says_it_is_in_the_top_folder() {
    let state = state();

    assert_eq!(state.shown_parent(&hit("/data/x.txt")), "(top folder)");
}

#[test]
fn a_result_outside_the_root_shows_its_full_parent() {
    let state = state();

    assert_eq!(state.shown_parent(&hit("/other/dir/x.txt")), "/other/dir");
}

#[test]
fn results_are_kept_sorted_by_path_whatever_order_they_arrive_in() {
    let mut state = searching();

    state.found(hit("/data/f198.txt"));
    state.found(hit("/data/f19.txt"));
    state.found(hit("/data/a/f2.txt"));

    let paths: Vec<_> = state.results().iter().map(|entry| entry.path.to_string()).collect();
    assert_eq!(paths, ["/data/a/f2.txt", "/data/f19.txt", "/data/f198.txt"]);
}

#[test]
fn the_selection_stays_on_the_same_result_when_an_earlier_one_arrives() {
    let mut state = searching();
    state.found(hit("/data/m.txt"));
    state.found(hit("/data/z.txt"));
    state.select(1);

    state.found(hit("/data/a.txt"));

    assert_eq!(state.selected().map(|entry| entry.name.clone()).as_deref(), Some("z.txt"));
    assert_eq!(state.selected_row(), Some(2));
}

fn previewing() -> SearchState {
    let mut state = SearchState::new(Location::Local, root()).with_preview(true);
    state.set_pattern("a".to_string());
    state
}

fn folder_hit(path: &str) -> Entry {
    Entry { is_dir: true, ..hit(path) }
}

#[test]
fn sync_preview_requests_the_selected_file() {
    let mut state = previewing();
    state.found(hit("/data/a.txt"));

    let outcome = state.sync_preview(7);

    assert_eq!(outcome, PreviewSync::Request { path: VfsPath::new("/data/a.txt") });
    assert!(matches!(state.preview(), Some(SearchPreview::File(file)) if file.is_loading()));
}

#[test]
fn sync_preview_keeps_when_the_selected_file_is_unchanged() {
    let mut state = previewing();
    state.found(hit("/data/b.txt"));
    state.sync_preview(1);
    state.found(hit("/data/a.txt"));

    assert_eq!(state.sync_preview(2), PreviewSync::Keep);
}

#[test]
fn sync_preview_shows_folder_and_cancels_for_a_directory() {
    let mut state = previewing();
    state.found(folder_hit("/data/sub"));

    assert_eq!(state.sync_preview(1), PreviewSync::Cancel);
    assert!(matches!(state.preview(), Some(SearchPreview::Folder(name)) if name == "sub"));
}

#[test]
fn sync_preview_is_inert_when_the_setting_is_off() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));

    assert_eq!(state.sync_preview(1), PreviewSync::Keep);
    assert!(state.preview().is_none());
}

#[test]
fn a_new_pattern_clears_the_preview_so_the_same_file_is_requested_again() {
    let mut state = previewing();
    state.found(hit("/data/a.txt"));
    state.sync_preview(1);

    state.set_pattern("ab".to_string());
    assert!(state.preview().is_none());
    state.found(hit("/data/a.txt"));

    assert_eq!(state.sync_preview(2), PreviewSync::Request { path: VfsPath::new("/data/a.txt") });
}

#[test]
fn preview_arrived_ignores_an_old_request_id() {
    let mut state = previewing();
    state.found(hit("/data/a.txt"));
    state.sync_preview(5);

    state.preview_arrived(4, Err("stale".to_string()));

    assert!(matches!(state.preview(), Some(SearchPreview::File(file)) if file.is_loading()));
}

#[test]
fn preview_arrived_fills_the_pane_for_the_current_request() {
    let mut state = previewing();
    state.found(hit("/data/a.txt"));
    state.sync_preview(5);

    state.preview_arrived(5, Err("denied".to_string()));

    assert!(matches!(state.preview(), Some(SearchPreview::File(file)) if !file.is_loading()));
}

#[test]
fn the_scope_names_the_place_and_the_folder_searched() {
    let state = state();

    assert_eq!(state.scope_text("this computer"), "this computer \u{b7} /data");
    assert_eq!(state.scope_text("prod"), "prod \u{b7} /data");
}

#[test]
fn a_search_that_skipped_folders_says_the_results_may_be_incomplete() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.finished(false, true);

    assert_eq!(state.status_text(), format!("1 found. {}", porthmos_core::SEARCH_INCOMPLETE_NOTE));
}

#[test]
fn an_empty_incomplete_search_does_not_claim_there_is_nothing() {
    let mut state = searching();
    state.finished(false, true);

    assert_eq!(state.status_text(), format!("Nothing found. {}", porthmos_core::SEARCH_INCOMPLETE_NOTE));
}

#[test]
fn a_complete_search_has_no_note() {
    let mut state = searching();
    state.found(hit("/data/a.txt"));
    state.finished(false, false);

    assert_eq!(state.status_text(), "1 found");
}

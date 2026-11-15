use porthmos_core::Location;

use super::{Entry, Group, MAX_HITS, PaletteState, Target};
use crate::shortcuts::Action;

fn command(label: &str, action: Action) -> Entry {
    Entry { group: Group::Commands, label: label.to_string(), hint: None, target: Target::Command(action) }
}

fn host(label: &str, name: &str) -> Entry {
    Entry { group: Group::Hosts, label: label.to_string(), hint: None, target: Target::Connect(name.to_string()) }
}

fn file(name: &str) -> Entry {
    Entry {
        group: Group::Files,
        label: name.to_string(),
        hint: Some("local".to_string()),
        target: Target::File { location: Location::Local, name: name.to_string() },
    }
}

fn sample() -> PaletteState {
    PaletteState::new(vec![
        command("Copy to other panel", Action::Copy),
        command("Delete", Action::Delete),
        command("Search", Action::Search),
        host("Connect alpha", "alpha"),
        host("Connect beta", "beta"),
        file("report.txt"),
        file("notes.md"),
    ])
}

fn labels(state: &PaletteState) -> Vec<String> {
    state.hits().into_iter().map(|entry| entry.label.clone()).collect()
}

#[test]
fn an_empty_query_lists_commands_and_hosts_in_their_given_order() {
    let state = sample();

    assert_eq!(
        labels(&state),
        ["Copy to other panel", "Delete", "Search", "Connect alpha", "Connect beta"].map(String::from)
    );
    assert_eq!(state.selected(), 0);
}

#[test]
fn a_query_ranks_the_better_match_first() {
    let mut state = sample();

    state.set_query("del".to_string());

    assert_eq!(labels(&state).first().map(String::as_str), Some("Delete"));
}

#[test]
fn the_prefixes_pick_one_group_each() {
    let mut state = sample();

    state.set_query(">se".to_string());
    assert_eq!(labels(&state), vec!["Search".to_string()]);

    state.set_query("@bet".to_string());
    assert_eq!(labels(&state), vec!["Connect beta".to_string()]);

    state.set_query("#rep".to_string());
    assert_eq!(labels(&state), vec!["report.txt".to_string()]);
}

#[test]
fn a_prefix_alone_lists_the_whole_group() {
    let mut state = sample();

    state.set_query("#".to_string());
    assert_eq!(labels(&state), ["report.txt", "notes.md"].map(String::from));

    state.set_query("@ ".to_string());
    assert_eq!(state.hit_count(), 2);

    state.set_query(">".to_string());
    assert_eq!(state.hit_count(), 3);
}

#[test]
fn files_are_not_listed_without_the_hash_prefix() {
    let mut state = sample();

    state.set_query("report".to_string());

    assert!(labels(&state).is_empty());
}

#[test]
fn hits_are_capped_at_the_maximum() {
    let entries = (0..500).map(|index| file(&format!("file-{index}.txt"))).collect();
    let mut state = PaletteState::new(entries);

    state.set_query("#".to_string());

    assert_eq!(state.hit_count(), MAX_HITS);
    assert_eq!(state.hits().len(), MAX_HITS);
}

#[test]
fn a_new_query_selects_the_first_hit() {
    let mut state = sample();
    state.move_selection(3);
    assert_eq!(state.selected(), 3);

    state.set_query("e".to_string());

    assert_eq!(state.selected(), 0);
}

#[test]
fn move_selection_clamps_at_both_ends() {
    let mut state = sample();

    state.move_selection(100);
    assert_eq!(state.selected(), 4);
    state.move_selection(-100);
    assert_eq!(state.selected(), 0);
}

#[test]
fn select_ignores_a_row_past_the_end() {
    let mut state = sample();
    state.select(2);

    state.select(5);
    state.select(100);

    assert_eq!(state.selected(), 2);
}

#[test]
fn a_query_with_no_match_has_no_selected_entry() {
    let mut state = sample();

    state.set_query("zzzzzz".to_string());

    assert_eq!(state.hit_count(), 0);
    assert!(state.selected_entry().is_none());
    state.move_selection(3);
    assert_eq!(state.selected(), 0);
}

#[test]
fn matching_is_case_insensitive() {
    let mut state = sample();

    state.set_query("DELETE".to_string());

    assert_eq!(labels(&state), vec!["Delete".to_string()]);
}

#[test]
fn the_selected_entry_follows_the_selection() {
    let mut state = sample();
    state.select(1);

    assert_eq!(state.selected_entry().map(|entry| entry.label.as_str()), Some("Delete"));
}

#[test]
fn the_query_is_kept_as_typed() {
    let mut state = sample();

    state.set_query("#  rep".to_string());

    assert_eq!(state.query(), "#  rep");
    assert_eq!(labels(&state), vec!["report.txt".to_string()]);
}

#[test]
fn replacing_the_entries_keeps_the_query_and_ranks_the_new_list() {
    let mut state = sample();
    state.set_query("@gam".to_string());
    assert_eq!(state.hit_count(), 0);

    let mut entries = vec![host("Connect gamma", "gamma"), host("Connect alpha", "alpha")];
    entries.push(command("Search", Action::Search));
    state.replace_entries(entries);

    assert_eq!(state.query(), "@gam");
    assert_eq!(state.hits().iter().map(|entry| entry.label.as_str()).collect::<Vec<_>>(), ["Connect gamma"]);
    assert_eq!(state.selected(), 0);
}

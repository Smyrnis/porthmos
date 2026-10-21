use std::collections::HashMap;

use super::*;
use crate::widgets::{frame::Hint, help::HelpSection};

fn keys_and_labels(hints: &Hints) -> Vec<(String, &'static str)> {
    hints.entries.iter().map(|hint| (hint.key.clone(), hint.label)).collect()
}

fn pair(key: &str, label: &'static str) -> (String, &'static str) {
    (key.to_string(), label)
}

fn remapped(overrides: &[(&str, &str)]) -> KeyBindings {
    let overrides: HashMap<String, String> =
        overrides.iter().map(|(action, key)| (action.to_string(), key.to_string())).collect();
    KeyBindings::from_overrides(&overrides).0
}

#[test]
fn files_hints_use_the_default_keys_in_order() {
    let hints = hints_for(Screen::Files, &KeyBindings::defaults());

    assert_eq!(
        keys_and_labels(&hints),
        vec![
            pair("F5", "copy"),
            pair("F7", "mkdir"),
            pair("F8", "delete"),
            pair("F9", "connections"),
            pair("F10", "quit"),
            pair("F3", "edit"),
            pair("F2", "rename"),
            pair("/", "filter"),
            pair("Space", "select"),
            pair("Tab", "panel"),
        ]
    );
    assert_eq!(hints.pinned, Some(Hint { key: "F1".to_string(), label: "help" }));
}

#[test]
fn files_has_no_right_slot() {
    assert_eq!(hints_for(Screen::Files, &KeyBindings::defaults()).right, None);
}

#[test]
fn a_remapped_action_shows_its_new_key() {
    let hints = hints_for(Screen::Files, &remapped(&[("copy", "ctrl+k")]));

    assert!(keys_and_labels(&hints).contains(&pair("Ctrl+K", "copy")));
    assert!(!keys_and_labels(&hints).contains(&pair("F5", "copy")));
}

#[test]
fn an_unbound_action_is_omitted_without_leaving_a_gap() {
    let hints = hints_for(Screen::Files, &remapped(&[("mkdir", "f5")]));

    let labels: Vec<&str> = hints.entries.iter().map(|hint| hint.label).collect();
    assert!(!labels.contains(&"copy"));
    assert!(keys_and_labels(&hints).contains(&pair("F5", "mkdir")));
    assert_eq!(hints.entries.len(), 9);
}

#[test]
fn removing_the_help_binding_leaves_no_pinned_pair() {
    let hints = hints_for(Screen::Files, &remapped(&[("open", "f1")]));

    assert_eq!(hints.pinned, None);
}

#[test]
fn a_remapped_help_key_is_shown_on_the_pinned_pair() {
    let hints = hints_for(Screen::Files, &remapped(&[("help", "ctrl+g")]));

    assert_eq!(hints.pinned, Some(Hint { key: "Ctrl+G".to_string(), label: "help" }));
}

#[test]
fn non_files_screens_put_back_in_the_right_slot() {
    let bindings = KeyBindings::defaults();

    for screen in [Screen::Connections, Screen::Search, Screen::Transfers, Screen::History, Screen::Sync] {
        let hints = hints_for(screen, &bindings);
        assert_eq!(hints.right, Some(Hint { key: "Esc".to_string(), label: "back" }), "{screen:?}");
        assert_eq!(hints.pinned, Some(Hint { key: "F1".to_string(), label: "help" }), "{screen:?}");
    }
}

#[test]
fn connections_history_and_sync_list_their_own_actions() {
    let bindings = KeyBindings::defaults();

    assert_eq!(
        keys_and_labels(&hints_for(Screen::Connections, &bindings)),
        vec![
            pair("Enter", "connect"),
            pair("F6", "new"),
            pair("F2", "edit"),
            pair("Delete", "delete"),
            pair("F8", "disconnect"),
        ]
    );
    assert_eq!(
        keys_and_labels(&hints_for(Screen::History, &bindings)),
        vec![pair("Enter", "details"), pair("F8", "clear")]
    );
    assert_eq!(keys_and_labels(&hints_for(Screen::Sync, &bindings)), vec![pair("Enter", "run"), pair("Space", "tick")]);
}

#[test]
fn search_has_only_the_pinned_help_and_back() {
    let bindings = KeyBindings::defaults();

    assert!(hints_for(Screen::Search, &bindings).entries.is_empty());
}

#[test]
fn transfers_list_retry_cancel_and_clear_done() {
    assert_eq!(
        keys_and_labels(&hints_for(Screen::Transfers, &KeyBindings::defaults())),
        vec![pair("Enter", "retry"), pair("F8", "cancel"), pair("Ctrl+R", "clear done")]
    );
}

#[test]
fn the_search_screen_hints_are_move_open_and_esc_close() {
    let hints = search_hints();

    assert_eq!(keys_and_labels(&hints), vec![pair("\u{2191}\u{2193}", "move"), pair("enter", "open")]);
    assert_eq!(hints.right, Some(Hint { key: "esc".to_string(), label: "close" }));
}

#[test]
fn the_search_hints_have_no_help_key_because_a_question_mark_is_typed() {
    assert_eq!(search_hints().pinned, None);
}

fn section<'a>(sections: &'a [HelpSection], title: &str) -> &'a HelpSection {
    sections.iter().find(|section| section.title == title).unwrap_or_else(|| panic!("no {title} section"))
}

fn rows_of(section: &HelpSection) -> Vec<(Option<&str>, &'static str)> {
    section.rows.iter().map(|row| (row.key.as_deref(), row.action)).collect()
}

#[test]
fn help_sections_follow_the_screens_in_order_then_global() {
    let sections = help_sections(&KeyBindings::defaults());

    let titles: Vec<&str> = sections.iter().map(|section| section.title).collect();
    assert_eq!(titles, ["files", "connections", "transfers", "history", "sync", "global"]);
}

#[test]
fn screen_sections_use_the_hint_labels_and_the_live_keys() {
    let sections = help_sections(&KeyBindings::defaults());

    let files = rows_of(section(&sections, "files"));
    assert!(files.contains(&(Some("F5"), "copy")));
    assert!(files.contains(&(Some("F10"), "quit")));
    let connections = rows_of(section(&sections, "connections"));
    assert_eq!(
        connections,
        vec![
            (Some("Enter"), "connect"),
            (Some("F6"), "new"),
            (Some("F2"), "edit"),
            (Some("Delete"), "delete"),
            (Some("F8"), "disconnect"),
        ]
    );
}

#[test]
fn a_remapped_key_shows_its_new_key() {
    let sections = help_sections(&remapped(&[("copy", "F6")]));

    assert!(rows_of(section(&sections, "files")).contains(&(Some("F6"), "copy")));
    assert!(!rows_of(section(&sections, "files")).contains(&(Some("F5"), "copy")));
}

#[test]
fn an_unbound_action_shows_no_key() {
    let sections = help_sections(&remapped(&[("copy", "F10")]));

    assert!(rows_of(section(&sections, "files")).contains(&(None, "quit")));
}

#[test]
fn global_lists_the_remaining_actions_by_their_config_names_and_one_movement_row() {
    let sections = help_sections(&KeyBindings::defaults());

    let global = rows_of(section(&sections, "global"));
    assert!(global.contains(&(Some("Ctrl+T"), "open_transfers")));
    assert!(global.contains(&(Some("Ctrl+Y"), "open_history")));
    assert!(global.contains(&(Some("Ctrl+U"), "open_sync")));
    assert!(global.contains(&(Some("F1"), "help")));
    assert!(global.contains(&(Some("Up/Down"), "move")));
    assert!(!global.iter().any(|(_, action)| *action == "up" || *action == "down"));
    assert!(!global.iter().any(|(_, action)| *action == "copy"));
}

#[test]
fn every_action_is_listed_somewhere_and_no_section_lists_an_action_twice() {
    use std::collections::HashSet;

    let listed: HashSet<Action> =
        [Screen::Files, Screen::Connections, Screen::Transfers, Screen::History, Screen::Sync]
            .iter()
            .flat_map(|screen| screen_actions(*screen).iter().map(|(action, _)| *action))
            .collect();
    let sections = help_sections(&KeyBindings::defaults());
    let global: HashSet<&str> = section(&sections, "global").rows.iter().map(|row| row.action).collect();

    for action in crate::input::ALL_ACTIONS {
        let movement = matches!(action, Action::Up | Action::Down);
        assert!(
            listed.contains(action) || global.contains(action.name()) || movement,
            "{} is not listed anywhere",
            action.name()
        );
    }
    for section in &sections {
        let actions: HashSet<_> = section.rows.iter().map(|row| row.action).collect();
        assert_eq!(actions.len(), section.rows.len(), "{} repeats an action", section.title);
    }
}

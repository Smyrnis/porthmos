use std::collections::HashMap;

use super::*;
use crate::widgets::frame::Hint;

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
fn transfers_and_search_have_only_the_pinned_help_and_back() {
    let bindings = KeyBindings::defaults();

    assert!(hints_for(Screen::Transfers, &bindings).entries.is_empty());
    assert!(hints_for(Screen::Search, &bindings).entries.is_empty());
}

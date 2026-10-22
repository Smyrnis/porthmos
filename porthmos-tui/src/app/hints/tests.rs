use std::collections::HashMap;

use super::*;
use crate::widgets::{frame::Hint, help::HelpSection};

fn keys_and_labels(hints: &Hints) -> Vec<(String, &'static str)> {
    hints.entries.iter().map(|hint| (hint.key.clone(), hint.label)).collect()
}

fn configured(text: &str) -> KeyBindings {
    let table: toml::Table = toml::from_str(text).unwrap();
    KeyBindings::from_frontend(&table).0
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
            pair("c", "copy"),
            pair("m", "move"),
            pair("n", "mkdir"),
            pair("d", "delete"),
            pair("F9", "connections"),
            pair("q", "quit"),
            pair("e", "edit"),
            pair("r", "rename"),
            pair("/", "filter"),
            pair("Space", "select"),
            pair("Tab", "panel"),
        ]
    );
    assert_eq!(hints.pinned, Some(Hint { key: "?".to_string(), label: "help" }));
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
    let hints = hints_for(Screen::Files, &configured("[keys]\ncopy = []\n"));

    let labels: Vec<&str> = hints.entries.iter().map(|hint| hint.label).collect();
    assert!(!labels.contains(&"copy"));
    assert_eq!(hints.entries.len(), 10);
}

#[test]
fn removing_the_help_binding_leaves_no_pinned_pair() {
    let hints = hints_for(Screen::Files, &configured("[keys]\nhelp = []\n"));

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
        assert_eq!(hints.pinned, Some(Hint { key: "?".to_string(), label: "help" }), "{screen:?}");
    }
}

#[test]
fn connections_history_and_sync_list_their_own_actions() {
    let bindings = KeyBindings::defaults();

    assert_eq!(
        keys_and_labels(&hints_for(Screen::Connections, &bindings)),
        vec![
            pair("Enter", "connect"),
            pair("n", "new"),
            pair("e", "edit"),
            pair("Delete", "delete"),
            pair("x", "disconnect"),
        ]
    );
    assert_eq!(
        keys_and_labels(&hints_for(Screen::History, &bindings)),
        vec![pair("Enter", "details"), pair("c", "clear")]
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
        vec![
            pair("p", "pause"),
            pair("K J", "reorder"),
            pair("r", "retry"),
            pair("x", "cancel"),
            pair("c", "clear done"),
            pair("Ctrl+L", "limit")
        ]
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
    assert!(files.contains(&(Some("c / F5"), "copy")));
    assert!(files.contains(&(Some("m / F6"), "move")));
    assert!(files.contains(&(Some("q / F10"), "quit")));
    let connections = rows_of(section(&sections, "connections"));
    assert_eq!(
        connections,
        vec![
            (Some("Enter"), "connect"),
            (Some("n / F6"), "new"),
            (Some("e / F2"), "edit"),
            (Some("Delete"), "delete"),
            (Some("x / F8"), "disconnect"),
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
    let sections = help_sections(&configured("[keys]\nquit = []\n"));

    assert!(rows_of(section(&sections, "files")).contains(&(None, "quit")));
}

#[test]
fn global_lists_the_remaining_actions_by_their_config_names_and_one_movement_row() {
    let sections = help_sections(&KeyBindings::defaults());

    let global = rows_of(section(&sections, "global"));
    assert!(global.contains(&(Some("Ctrl+T"), "open_transfers")));
    assert!(global.contains(&(Some("Ctrl+Y"), "open_history")));
    assert!(global.contains(&(Some("Ctrl+U"), "open_sync")));
    assert!(global.contains(&(Some("Ctrl+P"), "open_palette")));
    assert!(global.contains(&(Some("? / F1"), "help")));
    assert!(global.contains(&(Some("Up/Down"), "move cursor")));
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

fn files_rows(bindings: &KeyBindings) -> Vec<(Option<String>, &'static str)> {
    let sections = help_sections(bindings);
    section(&sections, "files").rows.iter().map(|row| (row.key.clone(), row.action)).collect()
}

fn row_key(rows: &[(Option<String>, &'static str)], action: &str) -> Option<String> {
    rows.iter().find(|(_, name)| *name == action).and_then(|(key, _)| key.clone())
}

#[test]
fn a_row_lists_the_letter_and_the_function_key() {
    let sections = help_sections(&KeyBindings::defaults());

    let files = rows_of(section(&sections, "files"));
    assert!(files.contains(&(Some("c / F5"), "copy")));
    assert!(files.contains(&(Some("d / F8"), "delete")));
    let transfers = rows_of(section(&sections, "transfers"));
    assert!(transfers.contains(&(Some("r / Enter"), "retry")));
    assert!(transfers.contains(&(Some("c / Ctrl+R"), "clear done")));
}

#[test]
fn an_alternate_added_in_the_config_shows_as_the_second_key() {
    let rows = files_rows(&configured("[keys.files]\ncopy = [\"f12\", \"c\"]\n"));

    assert_eq!(row_key(&rows, "copy").as_deref(), Some("F12 / c"));
}

#[test]
fn only_the_first_two_keys_are_shown() {
    let rows = files_rows(&configured("[keys.files]\ncopy = [\"c\", \"f5\", \"f12\"]\n"));

    assert_eq!(row_key(&rows, "copy").as_deref(), Some("c / F5"));
}

#[test]
fn a_remapped_key_replaces_the_defaults_in_the_row() {
    let rows = files_rows(&remapped(&[("copy", "f6")]));

    assert_eq!(row_key(&rows, "copy").as_deref(), Some("F6"));
}

#[test]
fn an_unbound_action_still_shows_no_key_in_its_row() {
    let rows = files_rows(&configured("[keys]\ncopy = []\n"));

    assert!(rows.contains(&(None, "copy")));
}

#[test]
fn the_files_section_has_a_one_to_nine_session_row() {
    let rows = files_rows(&KeyBindings::defaults());

    assert_eq!(row_key(&rows, "session").as_deref(), Some("1-9"));
    assert!(!rows.iter().any(|(_, action)| action.starts_with("session_")));
}

#[test]
fn a_session_row_with_remapped_keys_lists_the_bound_ones() {
    let rows = files_rows(&configured("[keys.files]\nsession_1 = \"a\"\nsession_5 = []\n"));

    assert_eq!(row_key(&rows, "session_1").as_deref(), Some("a"));
    assert_eq!(row_key(&rows, "session_2").as_deref(), Some("2"));
    assert_eq!(row_key(&rows, "session_5"), None);
    assert!(rows.iter().filter(|(_, action)| action.starts_with("session_")).count() == 8);
    assert_eq!(row_key(&rows, "session"), None);
}

#[test]
fn without_any_session_key_there_is_no_session_row() {
    let text: String = (1..=9).map(|number| format!("session_{number} = []\n")).collect();

    let rows = files_rows(&configured(&format!("[keys.files]\n{text}")));

    assert!(!rows.iter().any(|(_, action)| action.starts_with("session")));
}

#[test]
fn every_default_key_text_fits_the_help_key_column() {
    use unicode_width::UnicodeWidthStr;

    for section in help_sections(&KeyBindings::defaults()) {
        for row in &section.rows {
            let width = row.key.as_deref().map_or(0, |key| key.width());
            assert!(width <= 11, "{} {}: {:?}", section.title, row.action, row.key);
        }
    }
}

#[test]
fn hints_show_only_the_first_key_of_each_action() {
    let hints = hints_for(Screen::Files, &KeyBindings::defaults());

    assert!(keys_and_labels(&hints).contains(&pair("c", "copy")));
    assert!(!keys_and_labels(&hints).iter().any(|(key, _)| key.contains(" / ")));
}

#[test]
fn hints_never_show_a_key_that_belongs_to_another_screens_scope() {
    let bindings = KeyBindings::defaults();

    assert!(keys_and_labels(&hints_for(Screen::Files, &bindings)).contains(&pair("n", "mkdir")));
    assert!(keys_and_labels(&hints_for(Screen::Connections, &bindings)).contains(&pair("n", "new")));
    let transfers = keys_and_labels(&hints_for(Screen::Transfers, &bindings));
    assert!(!transfers.contains(&pair("c", "copy")));
    assert!(!transfers.contains(&pair("n", "mkdir")));
}

#[test]
fn the_form_hint_bar_lists_ctrl_t_test() {
    let hints = form_hints();

    assert!(keys_and_labels(&hints).contains(&pair("ctrl+t", "test")));
    assert!(keys_and_labels(&hints).contains(&pair("ctrl+s", "save")));
}

#[test]
fn the_transfers_hints_offer_the_speed_limit_with_its_key() {
    let hints = hints_for(Screen::Transfers, &KeyBindings::defaults());

    assert!(keys_and_labels(&hints).contains(&pair("Ctrl+L", "limit")));
}

#[test]
fn the_help_lists_the_speed_limit_on_the_transfers_section() {
    let sections = help_sections(&KeyBindings::defaults());

    assert!(rows_of(section(&sections, "transfers")).contains(&(Some("Ctrl+L"), "limit")));
}

#[test]
fn the_help_lists_both_reorder_keys_on_the_transfers_section() {
    let sections = help_sections(&KeyBindings::defaults());
    let rows = rows_of(section(&sections, "transfers"));

    assert!(rows.contains(&(Some("K"), "move earlier")));
    assert!(rows.contains(&(Some("J"), "move later")));
}

#[test]
fn an_unbound_reorder_key_leaves_the_other_one_alone_in_the_hint() {
    let hints = hints_for(Screen::Transfers, &configured("[keys.transfers]\nrow_down = []\n"));

    assert!(keys_and_labels(&hints).contains(&pair("K", "reorder")));
}

#[test]
fn both_reorder_keys_unbound_leave_no_reorder_hint() {
    let hints = hints_for(Screen::Transfers, &configured("[keys.transfers]\nrow_up = []\nrow_down = []\n"));

    assert!(keys_and_labels(&hints).iter().all(|(_, label)| *label != "reorder"));
}

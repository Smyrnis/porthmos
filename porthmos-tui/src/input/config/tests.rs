use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

use super::*;
use crate::input::{ALL_ACTIONS, format_key_spec};

const PRE_PHASE_KEYS: &str = r#"
[keys]
quit = "f10"
switch_panel = "tab"
up = "up"
down = "down"
open = "enter"
toggle_select = "space"
rename = "f2"
open_terminal = "f4"
copy = "f5"
add_connection = "f6"
mkdir = "f7"
delete = "f8"
open_connections = "f9"
delete_connection = "delete"
back = "esc"
refresh = "ctrl+r"
cancel_transfer = "ctrl+c"
toggle_hidden = "ctrl+h"
cycle_sort = "ctrl+s"
bookmark_here = "ctrl+d"
open_bookmarks = "ctrl+b"
open_search = "ctrl+f"
filter = "/"
open_transfers = "ctrl+t"
open_history = "ctrl+y"
open_sync = "ctrl+u"
edit = "f3"
cycle_session = "ctrl+n"
help = "f1"
"#;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn load(text: &str) -> (KeyBindings, Vec<String>) {
    let table: toml::Table = toml::from_str(text).unwrap();
    KeyBindings::from_frontend(&table)
}

fn keys(bindings: &KeyBindings, scope: Scope, action: Action) -> Vec<String> {
    bindings.keys_for(scope, action).into_iter().map(format_key_spec).collect()
}

#[test]
fn a_flat_string_replaces_the_action_in_every_scope_that_binds_it() {
    let (bindings, warnings) = load("[keys]\ndelete = \"f12\"\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    for scope in [Scope::Files, Scope::Transfers, Scope::History] {
        assert_eq!(keys(&bindings, scope, Action::Delete), ["F12"], "{scope:?}");
        assert_eq!(bindings.map_key(scope, key(KeyCode::F(8))), Action::Noop, "{scope:?}");
    }
    assert_eq!(keys(&bindings, Scope::Connections, Action::Disconnect), ["x", "F8"]);
}

#[test]
fn a_flat_entry_takes_a_key_from_the_global_scope_like_before() {
    let (bindings, warnings) = load("[keys]\ndelete = \"f9\"\n");

    assert_eq!(warnings, [r#"key "f9" is already bound to "open_connections"; "delete" now overrides it"#]);
    assert_eq!(bindings.key_for(Scope::Global, Action::OpenConnections), None);
}

#[test]
fn a_flat_list_binds_several_keys() {
    let (bindings, warnings) = load("[keys]\ncopy = [\"f5\", \"f12\"]\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(keys(&bindings, Scope::Files, Action::Copy), ["F5", "F12"]);
    assert_eq!(bindings.map_key(Scope::Files, key(KeyCode::F(12))), Action::Copy);
}

#[test]
fn an_empty_list_unbinds_the_action() {
    let (bindings, warnings) = load("[keys]\ncopy = []\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(keys(&bindings, Scope::Files, Action::Copy).is_empty());
    assert_eq!(bindings.map_key(Scope::Files, key(KeyCode::F(5))), Action::Noop);
}

#[test]
fn a_scope_table_replaces_only_that_scope() {
    let (bindings, warnings) = load("[keys.transfers]\ndelete = \"k\"\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(keys(&bindings, Scope::Transfers, Action::Delete), ["k"]);
    assert_eq!(keys(&bindings, Scope::Files, Action::Delete), ["d", "F8"]);
    assert_eq!(bindings.map_key(Scope::Transfers, key(KeyCode::Char('k'))), Action::Delete);
    assert_eq!(bindings.map_key(Scope::Transfers, key(KeyCode::F(8))), Action::Noop);
}

#[test]
fn a_scope_table_can_bind_an_action_the_scope_had_no_key_for() {
    let (bindings, warnings) = load("[keys.files]\nopen_history = \"h\"\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(keys(&bindings, Scope::Files, Action::OpenHistory), ["h", "Ctrl+Y"]);
    assert_eq!(bindings.map_key(Scope::Files, key(KeyCode::Char('h'))), Action::OpenHistory);
    assert_eq!(bindings.map_key(Scope::Connections, key(KeyCode::Char('h'))), Action::Noop);
}

#[test]
fn flat_entries_apply_before_scope_tables() {
    let (bindings, warnings) = load("[keys.files]\ncopy = [\"x\"]\n[keys]\ncopy = \"f12\"\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(keys(&bindings, Scope::Files, Action::Copy), ["x"]);
}

#[test]
fn the_pre_phase_example_config_loads_without_warnings_and_pins_each_action() {
    let table: toml::Table = toml::from_str(PRE_PHASE_KEYS).unwrap();
    let (bindings, warnings) = KeyBindings::from_frontend(&table);

    assert!(warnings.is_empty(), "{warnings:?}");
    let defaults = KeyBindings::defaults();
    let mut checked = 0;
    for (name, value) in table["keys"].as_table().unwrap() {
        let action = Action::from_name(name).unwrap_or_else(|| panic!("{name}"));
        let spec = parse_key_spec(value.as_str().unwrap()).unwrap();
        for scope in SCOPES {
            if defaults.0.get(scope).is_some_and(|actions| actions.contains_key(&action)) {
                assert_eq!(bindings.0[scope][&action], vec![spec], "{scope:?} {name}");
                checked += 1;
            }
        }
    }
    assert!(checked >= 30, "{checked}");
}

#[test]
fn an_unknown_scope_warns_and_the_rest_still_applies() {
    let (bindings, warnings) = load("[keys]\nquit = \"ctrl+q\"\n[keys.palette]\nquit = \"z\"\n");

    assert_eq!(warnings, [r#"unknown key scope "palette""#]);
    assert_eq!(keys(&bindings, Scope::Global, Action::Quit), ["Ctrl+Q"]);
}

#[test]
fn an_unknown_action_in_a_scope_table_warns() {
    let (_, warnings) = load("[keys.files]\nfrobnicate = \"x\"\n");

    assert_eq!(warnings, [r#"unknown key binding action "frobnicate""#]);
}

#[test]
fn a_wrong_value_type_warns_and_keeps_the_defaults_for_that_action() {
    let (bindings, warnings) = load("[keys]\nquit = 3\ncopy = true\nmkdir = [\"f7\", 3]\n");

    let message = |name: &str| format!(r#"key binding "{name}" must be a string, a list of strings or a table"#);
    assert_eq!(warnings, [message("copy"), message("mkdir"), message("quit")]);
    assert_eq!(keys(&bindings, Scope::Global, Action::Quit), ["q", "F10"]);
    assert_eq!(keys(&bindings, Scope::Files, Action::Copy), ["c", "F5"]);
    assert_eq!(keys(&bindings, Scope::Files, Action::Mkdir), ["n", "F7"]);
}

#[test]
fn a_wrong_value_type_inside_a_scope_table_warns() {
    let (bindings, warnings) = load("[keys.files]\ncopy = 3\n");

    assert_eq!(warnings, [r#"key binding "copy" must be a string, a list of strings or a table"#]);
    assert_eq!(keys(&bindings, Scope::Files, Action::Copy), ["c", "F5"]);
}

#[test]
fn keys_that_is_not_a_table_warns_and_uses_the_defaults() {
    let (bindings, warnings) = load("keys = 3\n");

    assert_eq!(warnings, ["[keys] must be a table; using the default key bindings"]);
    assert_eq!(keys(&bindings, Scope::Global, Action::Quit), ["q", "F10"]);
}

#[test]
fn an_invalid_key_in_a_list_warns_and_keeps_the_valid_ones() {
    let (bindings, warnings) = load("[keys]\ncopy = [\"f12\", \"nope\"]\n");

    assert_eq!(warnings, [r#"invalid key "nope" for "copy": unrecognized key "nope""#]);
    assert_eq!(keys(&bindings, Scope::Files, Action::Copy), ["F12"]);
}

#[test]
fn a_list_with_only_invalid_keys_keeps_the_defaults() {
    let (bindings, warnings) = load("[keys]\ncopy = [\"nope\"]\n");

    assert_eq!(warnings.len(), 1);
    assert_eq!(keys(&bindings, Scope::Files, Action::Copy), ["c", "F5"]);
}

#[test]
fn the_same_key_for_two_actions_in_one_scope_warns_and_the_later_wins() {
    let (bindings, warnings) = load("[keys.files]\ncopy = \"x\"\nmkdir = \"x\"\n");

    assert_eq!(warnings, [r#"key "x" is already bound to "copy"; "mkdir" now overrides it"#]);
    assert_eq!(bindings.map_key(Scope::Files, key(KeyCode::Char('x'))), Action::Mkdir);
    assert!(keys(&bindings, Scope::Files, Action::Copy).is_empty());
}

#[test]
fn the_same_key_in_two_scopes_is_allowed_without_a_warning() {
    let (bindings, warnings) = load("[keys.files]\nmkdir = \"z\"\n[keys.connections]\nrename = \"z\"\n");

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(bindings.map_key(Scope::Files, key(KeyCode::Char('z'))), Action::Mkdir);
    assert_eq!(bindings.map_key(Scope::Connections, key(KeyCode::Char('z'))), Action::Rename);
}

#[test]
fn warnings_and_results_are_deterministic_across_runs() {
    let text = "[keys]\nquit = \"f1\"\nhelp = \"f1\"\ncopy = \"f12\"\nmkdir = \"f12\"\n[keys.files]\ndelete = \"x\"\nrename = \"x\"\n";

    let (first, first_warnings) = load(text);
    for _ in 0..20 {
        let (again, warnings) = load(text);
        assert_eq!(warnings, first_warnings);
        for scope in SCOPES {
            for action in ALL_ACTIONS {
                assert_eq!(keys(&again, *scope, *action), keys(&first, *scope, *action));
            }
        }
    }
}

#[test]
fn the_old_warning_texts_are_unchanged() {
    let (_, warnings) = load("[keys]\nfrobnicate = \"f10\"\nquit = \"not-a-key\"\nrefresh = \"f10\"\n");

    assert_eq!(
        warnings,
        [
            r#"unknown key binding action "frobnicate""#,
            r#"invalid key "not-a-key" for "quit": unrecognized key "not-a-key""#,
            r#"key "f10" is already bound to "quit"; "refresh" now overrides it"#,
        ]
    );
}

#[test]
fn from_overrides_still_takes_a_string_map() {
    let overrides = HashMap::from([("quit".to_string(), "ctrl+q".to_string())]);

    let (bindings, warnings) = KeyBindings::from_overrides(&overrides);

    assert!(warnings.is_empty());
    assert_eq!(keys(&bindings, Scope::Global, Action::Quit), ["Ctrl+Q"]);
}

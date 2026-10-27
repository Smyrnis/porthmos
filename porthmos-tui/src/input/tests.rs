use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

use super::*;

fn key(code: KeyCode) -> KeyEvent {
    key_with_modifiers(code, KeyModifiers::NONE)
}

fn key_with_modifiers(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::NONE }
}

fn map_key_via_defaults(scope: Scope, code: KeyCode) -> Action {
    KeyBindings::defaults().map_key(scope, key(code))
}

fn map_key_via_defaults_with_modifiers(scope: Scope, code: KeyCode, modifiers: KeyModifiers) -> Action {
    KeyBindings::defaults().map_key(scope, key_with_modifiers(code, modifiers))
}

#[test]
fn format_key_spec_renders_a_plain_function_key() {
    assert_eq!(format_key_spec(KeySpec { code: KeyCode::F(10), modifiers: KeyModifiers::NONE }), "F10");
}

#[test]
fn format_key_spec_renders_a_plain_letter_as_typed_and_a_modified_one_in_capitals() {
    assert_eq!(format_key_spec(KeySpec { code: KeyCode::Char('c'), modifiers: KeyModifiers::NONE }), "c");
    assert_eq!(format_key_spec(KeySpec { code: KeyCode::Char('?'), modifiers: KeyModifiers::NONE }), "?");
    assert_eq!(format_key_spec(KeySpec { code: KeyCode::Char('c'), modifiers: KeyModifiers::ALT }), "Alt+C");
}

#[test]
fn format_key_spec_renders_a_control_modifier() {
    assert_eq!(format_key_spec(KeySpec { code: KeyCode::Char('r'), modifiers: KeyModifiers::CONTROL }), "Ctrl+R");
}

fn ctrl(character: char) -> KeyEvent {
    key_with_modifiers(KeyCode::Char(character), KeyModifiers::CONTROL)
}

fn default_keys() -> Vec<(Scope, Action, KeyEvent)> {
    vec![
        (Scope::Global, Action::Quit, key(KeyCode::F(10))),
        (Scope::Global, Action::Up, key(KeyCode::Up)),
        (Scope::Global, Action::Down, key(KeyCode::Down)),
        (Scope::Global, Action::Open, key(KeyCode::Enter)),
        (Scope::Global, Action::ToggleSelect, key(KeyCode::Char(' '))),
        (Scope::Global, Action::Back, key(KeyCode::Esc)),
        (Scope::Global, Action::Help, key(KeyCode::F(1))),
        (Scope::Global, Action::Filter, key(KeyCode::Char('/'))),
        (Scope::Global, Action::OpenConnections, key(KeyCode::F(9))),
        (Scope::Global, Action::OpenTerminal, key(KeyCode::F(4))),
        (Scope::Global, Action::AddConnection, key(KeyCode::F(6))),
        (Scope::Global, Action::Refresh, ctrl('r')),
        (Scope::Global, Action::CancelTransfer, ctrl('c')),
        (Scope::Global, Action::ToggleHidden, ctrl('h')),
        (Scope::Global, Action::CycleSort, ctrl('s')),
        (Scope::Global, Action::BookmarkHere, ctrl('d')),
        (Scope::Global, Action::OpenBookmarks, ctrl('b')),
        (Scope::Global, Action::OpenSearch, ctrl('f')),
        (Scope::Global, Action::OpenTransfers, ctrl('t')),
        (Scope::Global, Action::OpenHistory, ctrl('y')),
        (Scope::Global, Action::OpenSync, ctrl('u')),
        (Scope::Global, Action::CycleSession, ctrl('n')),
        (Scope::Files, Action::SwitchPanel, key(KeyCode::Tab)),
        (Scope::Files, Action::Rename, key(KeyCode::F(2))),
        (Scope::Files, Action::Edit, key(KeyCode::F(3))),
        (Scope::Files, Action::Copy, key(KeyCode::F(5))),
        (Scope::Files, Action::Mkdir, key(KeyCode::F(7))),
        (Scope::Files, Action::Delete, key(KeyCode::F(8))),
        (Scope::Connections, Action::Rename, key(KeyCode::F(2))),
        (Scope::Connections, Action::Disconnect, key(KeyCode::F(8))),
        (Scope::Connections, Action::DeleteConnection, key(KeyCode::Delete)),
        (Scope::Transfers, Action::Delete, key(KeyCode::F(8))),
        (Scope::History, Action::Delete, key(KeyCode::F(8))),
    ]
}

#[test]
fn every_default_key_still_maps_to_its_action_on_the_screens_it_worked_on() {
    let bindings = KeyBindings::defaults();
    for (scope, action, event) in default_keys() {
        assert_eq!(bindings.map_key(scope, event), action, "{scope:?} {action:?}");
        if scope == Scope::Global {
            for other in SCOPES {
                if *other == Scope::Files && event == key(KeyCode::F(6)) {
                    continue;
                }
                assert_eq!(bindings.map_key(*other, event), action, "{other:?} {action:?}");
            }
        }
    }
}

#[test]
fn file_operation_keys_do_not_map_outside_the_files_scope() {
    let bindings = KeyBindings::defaults();
    for code in [KeyCode::Tab, KeyCode::F(3), KeyCode::F(5), KeyCode::F(7)] {
        for scope in [Scope::Connections, Scope::Transfers, Scope::History, Scope::Sync] {
            assert_eq!(bindings.map_key(scope, key(code)), Action::Noop, "{scope:?} {code:?}");
        }
    }
}

#[test]
fn scope_names_round_trip_and_unknown_names_are_rejected() {
    for scope in SCOPES {
        assert_eq!(Scope::from_name(scope.name()), Some(*scope));
    }
    assert_eq!(SCOPES.len(), 6);
    assert_eq!(Scope::from_name("palette"), None);
    assert_eq!(Scope::from_name("Files"), None);
}

#[test]
fn session_action_names_round_trip_for_one_to_nine_only() {
    for number in 1..=9u8 {
        let action = Action::JumpToSession(number);
        assert_eq!(action.name(), format!("session_{number}"));
        assert_eq!(Action::from_name(action.name()), Some(action));
    }
    for name in ["session_0", "session_10", "session_", "session", "session_a"] {
        assert_eq!(Action::from_name(name), None, "{name}");
    }
    assert_eq!(SESSION_ACTIONS.len(), 9);
}

#[test]
fn disconnect_round_trips_by_name() {
    assert_eq!(Action::Disconnect.name(), "disconnect");
    assert_eq!(Action::from_name("disconnect"), Some(Action::Disconnect));
}

#[test]
fn all_actions_lists_disconnect_but_not_the_session_actions() {
    assert!(ALL_ACTIONS.contains(&Action::Disconnect));
    assert!(SESSION_ACTIONS.iter().all(|action| !ALL_ACTIONS.contains(action)));
}

fn bound(entries: &[(Scope, Action, KeyEvent)]) -> KeyBindings {
    let mut map: HashMap<Scope, HashMap<Action, Vec<KeySpec>>> = HashMap::new();
    for (scope, action, event) in entries {
        let spec = KeySpec { code: event.code, modifiers: event.modifiers };
        map.entry(*scope).or_default().entry(*action).or_default().push(spec);
    }
    KeyBindings(map)
}

#[test]
fn a_scope_key_wins_over_the_same_global_key() {
    let bindings = bound(&[
        (Scope::Global, Action::Open, key(KeyCode::Char('r'))),
        (Scope::Transfers, Action::Delete, key(KeyCode::Char('r'))),
    ]);

    assert_eq!(bindings.map_key(Scope::Transfers, key(KeyCode::Char('r'))), Action::Delete);
    assert_eq!(bindings.map_key(Scope::Files, key(KeyCode::Char('r'))), Action::Open);
}

#[test]
fn keys_for_lists_the_scope_keys_then_the_global_ones_without_duplicates() {
    let bindings = bound(&[
        (Scope::Transfers, Action::Refresh, key(KeyCode::Char('c'))),
        (Scope::Global, Action::Refresh, ctrl('r')),
        (Scope::Global, Action::Refresh, key(KeyCode::Char('c'))),
    ]);

    let keys: Vec<String> =
        bindings.keys_for(Scope::Transfers, Action::Refresh).into_iter().map(format_key_spec).collect();

    assert_eq!(keys, ["c", "Ctrl+R"]);
    let global: Vec<String> =
        bindings.keys_for(Scope::Global, Action::Refresh).into_iter().map(format_key_spec).collect();
    assert_eq!(global, ["Ctrl+R", "c"]);
}

#[test]
fn key_for_is_the_first_of_keys_for_and_an_unbound_action_has_none() {
    let bindings = KeyBindings::defaults();

    assert_eq!(
        bindings.key_for(Scope::Files, Action::Copy),
        bindings.keys_for(Scope::Files, Action::Copy).first().copied()
    );
    assert_eq!(bindings.key_for(Scope::Files, Action::Disconnect), None);
    assert!(bindings.keys_for(Scope::Files, Action::Disconnect).is_empty());
    assert_eq!(bindings.key_for(Scope::Connections, Action::Quit), bindings.key_for(Scope::Global, Action::Quit));
}

#[test]
fn defaults_never_bind_two_actions_to_the_same_key_within_a_scope() {
    let bindings = KeyBindings::defaults();
    for (scope, actions) in &bindings.0 {
        let mut seen: Vec<KeySpec> = Vec::new();
        for (action, keys) in actions {
            for spec in keys {
                assert!(!seen.contains(spec), "{scope:?}: {action:?} shares {spec:?} with another action");
                seen.push(*spec);
            }
        }
    }
}

#[test]
fn shift_is_ignored_for_character_keys() {
    let (bindings, warnings) = KeyBindings::from_overrides(&HashMap::from([("help".to_string(), "?".to_string())]));
    assert!(warnings.is_empty(), "{warnings:?}");

    let plain = key(KeyCode::Char('?'));
    let shifted = key_with_modifiers(KeyCode::Char('?'), KeyModifiers::SHIFT);
    assert_eq!(bindings.map_key(Scope::Files, plain), Action::Help);
    assert_eq!(bindings.map_key(Scope::Files, shifted), Action::Help);

    let (letters, _) = KeyBindings::from_overrides(&HashMap::from([("quit".to_string(), "q".to_string())]));
    assert_eq!(letters.map_key(Scope::Files, key(KeyCode::Char('q'))), Action::Quit);
    assert_eq!(
        letters.map_key(Scope::Files, key_with_modifiers(KeyCode::Char('Q'), KeyModifiers::SHIFT)),
        Action::Noop
    );
    assert_eq!(letters.map_key(Scope::Files, key(KeyCode::Char('Q'))), Action::Noop);

    assert_eq!(parse_key_spec("shift+?").unwrap(), parse_key_spec("?").unwrap());
    assert_eq!(parse_key_spec("shift+tab").unwrap().modifiers, KeyModifiers::SHIFT);
}

#[test]
fn an_unbound_key_maps_to_noop() {
    let bindings = KeyBindings::defaults();
    assert_eq!(bindings.map_key(Scope::Global, key(KeyCode::Char('x'))), Action::Noop);
}

#[test]
fn parse_key_spec_parses_plain_function_keys() {
    assert_eq!(parse_key_spec("F10").unwrap(), KeySpec { code: KeyCode::F(10), modifiers: KeyModifiers::NONE });
}

#[test]
fn parse_key_spec_parses_ctrl_modifier_case_insensitively() {
    assert_eq!(
        parse_key_spec("Ctrl+R").unwrap(),
        KeySpec { code: KeyCode::Char('r'), modifiers: KeyModifiers::CONTROL }
    );
}

#[test]
fn parse_key_spec_parses_named_keys() {
    assert_eq!(parse_key_spec("tab").unwrap().code, KeyCode::Tab);
    assert_eq!(parse_key_spec("space").unwrap().code, KeyCode::Char(' '));
    assert_eq!(parse_key_spec("esc").unwrap().code, KeyCode::Esc);
}

#[test]
fn parse_key_spec_rejects_unrecognized_keys() {
    assert!(parse_key_spec("banana").is_err());
    assert!(parse_key_spec("f99").is_err());
}

#[test]
fn key_bindings_defaults_maps_f10_to_quit() {
    let bindings = KeyBindings::defaults();
    assert_eq!(bindings.map_key(Scope::Global, key(KeyCode::F(10))), Action::Quit);
}

#[test]
fn from_overrides_applies_a_valid_override() {
    let mut overrides = HashMap::new();
    overrides.insert("quit".to_string(), "ctrl+q".to_string());

    let (bindings, warnings) = KeyBindings::from_overrides(&overrides);

    assert!(warnings.is_empty());
    assert_eq!(
        bindings.map_key(Scope::Global, key_with_modifiers(KeyCode::Char('q'), KeyModifiers::CONTROL)),
        Action::Quit
    );
}

#[test]
fn from_overrides_warns_on_unknown_action_name() {
    let mut overrides = HashMap::new();
    overrides.insert("frobnicate".to_string(), "F10".to_string());

    let (_, warnings) = KeyBindings::from_overrides(&overrides);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("frobnicate"));
}

#[test]
fn from_overrides_warns_on_unparseable_key_string_and_keeps_the_default() {
    let mut overrides = HashMap::new();
    overrides.insert("quit".to_string(), "not-a-key".to_string());

    let (bindings, warnings) = KeyBindings::from_overrides(&overrides);

    assert_eq!(warnings.len(), 1);
    assert_eq!(bindings.map_key(Scope::Global, key(KeyCode::F(10))), Action::Quit);
}

#[test]
fn from_overrides_warns_on_duplicate_binding() {
    let mut overrides = HashMap::new();
    overrides.insert("refresh".to_string(), "F10".to_string());

    let (_, warnings) = KeyBindings::from_overrides(&overrides);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("F10"));
}

#[test]
fn from_overrides_resolves_a_colliding_binding_deterministically() {
    let mut overrides = HashMap::new();
    overrides.insert("refresh".to_string(), "F10".to_string());

    let (bindings, warnings) = KeyBindings::from_overrides(&overrides);

    assert_eq!(warnings.len(), 1);
    assert_eq!(bindings.map_key(Scope::Global, key(KeyCode::F(10))), Action::Refresh);
    assert_eq!(bindings.keys_for(Scope::Global, Action::Quit).len(), 1);
    assert_eq!(bindings.key_for(Scope::Global, Action::Quit).map(format_key_spec).as_deref(), Some("q"));
}

#[test]
fn action_name_and_from_name_round_trip() {
    for action in [Action::Quit, Action::ToggleHidden, Action::CycleSort, Action::Back, Action::OpenTransfers] {
        assert_eq!(Action::from_name(action.name()), Some(action));
    }
}

#[test]
fn from_name_rejects_noop() {
    assert_eq!(Action::from_name("noop"), None);
}

#[test]
fn f1_maps_to_help() {
    assert_eq!(map_key_via_defaults(Scope::Global, KeyCode::F(1)), Action::Help);
}

#[test]
fn f6_is_bound_to_add_connection() {
    assert_eq!(map_key_via_defaults(Scope::Global, KeyCode::F(6)), Action::AddConnection);
}

#[test]
fn ctrl_d_maps_to_bookmark_here() {
    assert_eq!(
        map_key_via_defaults_with_modifiers(Scope::Global, KeyCode::Char('d'), KeyModifiers::CONTROL),
        Action::BookmarkHere
    );
}

#[test]
fn ctrl_b_maps_to_open_bookmarks() {
    assert_eq!(
        map_key_via_defaults_with_modifiers(Scope::Global, KeyCode::Char('b'), KeyModifiers::CONTROL),
        Action::OpenBookmarks
    );
}

#[test]
fn ctrl_f_maps_to_open_search() {
    assert_eq!(
        map_key_via_defaults_with_modifiers(Scope::Global, KeyCode::Char('f'), KeyModifiers::CONTROL),
        Action::OpenSearch
    );
}

#[test]
fn ctrl_n_maps_to_cycle_session() {
    assert_eq!(
        map_key_via_defaults_with_modifiers(Scope::Global, KeyCode::Char('n'), KeyModifiers::CONTROL),
        Action::CycleSession
    );
}

#[test]
fn all_actions_excludes_noop() {
    assert!(!ALL_ACTIONS.contains(&Action::Noop));
}

#[test]
fn delete_key_is_bound_to_delete_connection() {
    assert_eq!(map_key_via_defaults(Scope::Connections, KeyCode::Delete), Action::DeleteConnection);
}

#[test]
fn ctrl_t_maps_to_open_transfers() {
    assert_eq!(
        map_key_via_defaults_with_modifiers(Scope::Global, KeyCode::Char('t'), KeyModifiers::CONTROL),
        Action::OpenTransfers
    );
    assert_eq!(Action::OpenTransfers.name(), "open_transfers");
}

#[test]
fn from_frontend_reads_the_keys_table() {
    let frontend: toml::Table = toml::from_str("[keys]\nquit = \"ctrl+q\"\n").unwrap();

    let (bindings, warnings) = KeyBindings::from_frontend(&frontend);

    assert!(warnings.is_empty());
    assert_eq!(
        bindings.key_for(Scope::Global, Action::Quit),
        Some(KeySpec { code: KeyCode::Char('q'), modifiers: KeyModifiers::CONTROL })
    );
}

#[test]
fn from_frontend_without_a_keys_table_uses_the_defaults() {
    let (bindings, warnings) = KeyBindings::from_frontend(&toml::Table::new());

    assert!(warnings.is_empty());
    assert_eq!(
        bindings.key_for(Scope::Global, Action::Quit),
        KeyBindings::defaults().key_for(Scope::Global, Action::Quit)
    );
}

#[test]
fn from_frontend_with_a_non_string_key_warns_and_keeps_the_defaults() {
    let frontend: toml::Table = toml::from_str("[keys]\nquit = 3\n").unwrap();

    let (bindings, warnings) = KeyBindings::from_frontend(&frontend);

    assert_eq!(warnings, [r#"key binding "quit" must be a string, a list of strings or a table"#]);
    assert_eq!(
        bindings.key_for(Scope::Global, Action::Quit),
        KeyBindings::defaults().key_for(Scope::Global, Action::Quit)
    );
}

fn letter(character: char) -> KeyEvent {
    key(KeyCode::Char(character))
}

fn new_default_keys() -> Vec<(Scope, Action, KeyEvent)> {
    let mut keys = vec![
        (Scope::Global, Action::Quit, letter('q')),
        (Scope::Global, Action::Help, letter('?')),
        (Scope::Global, Action::Up, letter('k')),
        (Scope::Global, Action::Down, letter('j')),
        (Scope::Files, Action::Copy, letter('c')),
        (Scope::Files, Action::Mkdir, letter('n')),
        (Scope::Files, Action::Delete, letter('d')),
        (Scope::Files, Action::Rename, letter('r')),
        (Scope::Files, Action::Edit, letter('e')),
        (Scope::Files, Action::OpenTransfers, letter('t')),
        (Scope::Connections, Action::Rename, letter('e')),
        (Scope::Connections, Action::AddConnection, letter('n')),
        (Scope::Connections, Action::Disconnect, letter('x')),
        (Scope::Transfers, Action::Open, letter('r')),
        (Scope::Transfers, Action::Delete, letter('x')),
        (Scope::Transfers, Action::Refresh, letter('c')),
        (Scope::History, Action::Delete, letter('c')),
    ];
    for number in 1..=9u8 {
        keys.push((Scope::Files, Action::JumpToSession(number), letter(char::from(b'0' + number))));
    }
    keys
}

#[test]
fn the_new_default_keys_map_on_their_screens() {
    let bindings = KeyBindings::defaults();
    for (scope, action, event) in new_default_keys() {
        assert_eq!(bindings.map_key(scope, event), action, "{scope:?} {action:?}");
        if scope == Scope::Global {
            for other in SCOPES {
                assert_eq!(bindings.map_key(*other, event), action, "{other:?} {action:?}");
            }
        }
    }
}

#[test]
fn the_new_letters_come_before_the_function_keys() {
    let bindings = KeyBindings::defaults();
    let shown = |scope, action| bindings.keys_for(scope, action).into_iter().map(format_key_spec).collect::<Vec<_>>();

    assert_eq!(shown(Scope::Files, Action::Copy), ["c", "F5"]);
    assert_eq!(shown(Scope::Global, Action::Help), ["?", "F1"]);
    assert_eq!(shown(Scope::Global, Action::Quit), ["q", "F10"]);
    assert_eq!(shown(Scope::Connections, Action::Disconnect), ["x", "F8"]);
    assert_eq!(shown(Scope::Transfers, Action::Refresh), ["c", "Ctrl+R"]);
    assert_eq!(shown(Scope::Transfers, Action::Open), ["r", "Enter"]);
    assert_eq!(shown(Scope::Connections, Action::AddConnection), ["n", "F6"]);
    assert_eq!(shown(Scope::Global, Action::Up), ["Up", "k"]);
}

#[test]
fn letters_do_not_leak_into_screens_that_do_not_bind_them() {
    let bindings = KeyBindings::defaults();
    for scope in [Scope::Connections, Scope::Transfers, Scope::History, Scope::Sync] {
        for character in ['d', 't', '1', '5'] {
            assert_eq!(bindings.map_key(scope, letter(character)), Action::Noop, "{scope:?} {character}");
        }
    }
    assert_eq!(bindings.map_key(Scope::Connections, letter('c')), Action::Noop);
    assert_eq!(bindings.map_key(Scope::Transfers, letter('n')), Action::Noop);
}

#[test]
fn uppercase_letters_trigger_nothing() {
    let bindings = KeyBindings::defaults();
    for scope in SCOPES {
        for character in ['C', 'D', 'Q', 'N', 'R', 'E', 'T', 'X', 'J', 'K'] {
            if *scope == Scope::Transfers && matches!(character, 'J' | 'K') {
                continue;
            }
            assert_eq!(bindings.map_key(*scope, letter(character)), Action::Noop, "{scope:?} {character}");
            let shifted = key_with_modifiers(KeyCode::Char(character), KeyModifiers::SHIFT);
            assert_eq!(bindings.map_key(*scope, shifted), Action::Noop, "{scope:?} shift {character}");
        }
    }
}

#[test]
fn the_question_mark_maps_with_and_without_shift() {
    let bindings = KeyBindings::defaults();

    assert_eq!(bindings.map_key(Scope::Files, letter('?')), Action::Help);
    assert_eq!(
        bindings.map_key(Scope::Files, key_with_modifiers(KeyCode::Char('?'), KeyModifiers::SHIFT)),
        Action::Help
    );
}

#[test]
fn the_example_config_binds_every_action_to_its_default_keys() {
    let frontend: toml::Table = toml::from_str(include_str!("../../../config/config.example.toml")).unwrap();

    let (bindings, warnings) = KeyBindings::from_frontend(&frontend);

    assert!(warnings.is_empty(), "{warnings:?}");
    let defaults = KeyBindings::defaults();
    for scope in SCOPES {
        for action in ALL_ACTIONS.iter().chain(SESSION_ACTIONS) {
            assert_eq!(
                bindings.keys_for(*scope, *action),
                defaults.keys_for(*scope, *action),
                "{scope:?} {}",
                action.name()
            );
        }
    }
}

#[test]
fn the_example_config_has_a_table_for_every_scope_that_has_default_keys() {
    let frontend: toml::Table = toml::from_str(include_str!("../../../config/config.example.toml")).unwrap();
    let keys = frontend["keys"].as_table().unwrap();

    for scope in [Scope::Global, Scope::Files, Scope::Connections, Scope::Transfers, Scope::History] {
        assert!(keys.get(scope.name()).is_some_and(toml::Value::is_table), "{scope:?}");
    }
}

#[test]
fn the_filter_action_is_named_filter() {
    assert_eq!(Action::Filter.name(), "filter");
    assert!(ALL_ACTIONS.contains(&Action::Filter));
}

#[test]
fn ctrl_y_maps_to_open_history() {
    assert_eq!(
        map_key_via_defaults_with_modifiers(Scope::Global, KeyCode::Char('y'), KeyModifiers::CONTROL),
        Action::OpenHistory
    );
    assert_eq!(Action::OpenHistory.name(), "open_history");
    assert_eq!(Action::from_name("open_history"), Some(Action::OpenHistory));
    assert!(ALL_ACTIONS.contains(&Action::OpenHistory));
}

#[test]
fn f3_maps_to_edit() {
    assert_eq!(map_key_via_defaults(Scope::Files, KeyCode::F(3)), Action::Edit);
    assert_eq!(Action::Edit.name(), "edit");
    assert_eq!(Action::from_name("edit"), Some(Action::Edit));
    assert!(ALL_ACTIONS.contains(&Action::Edit));
}

#[test]
fn open_sync_is_bound_to_ctrl_u_and_round_trips_by_name() {
    assert_eq!(
        map_key_via_defaults_with_modifiers(Scope::Global, KeyCode::Char('u'), KeyModifiers::CONTROL),
        Action::OpenSync
    );
    assert_eq!(Action::OpenSync.name(), "open_sync");
    assert_eq!(Action::from_name("open_sync"), Some(Action::OpenSync));
    assert!(ALL_ACTIONS.contains(&Action::OpenSync));
}

#[test]
fn ctrl_p_maps_to_open_palette_on_every_screen() {
    for scope in SCOPES {
        assert_eq!(
            KeyBindings::defaults().map_key(*scope, key_with_modifiers(KeyCode::Char('p'), KeyModifiers::CONTROL)),
            Action::OpenPalette,
            "{scope:?}"
        );
    }
}

#[test]
fn open_palette_round_trips_by_name_and_is_listed() {
    assert_eq!(Action::OpenPalette.name(), "open_palette");
    assert_eq!(Action::from_name("open_palette"), Some(Action::OpenPalette));
    assert!(ALL_ACTIONS.contains(&Action::OpenPalette));
}

#[test]
fn move_is_m_and_f6_on_the_files_screen() {
    let bindings = KeyBindings::defaults();

    assert_eq!(bindings.map_key(Scope::Files, letter('m')), Action::Move);
    assert_eq!(bindings.map_key(Scope::Files, key(KeyCode::F(6))), Action::Move);
}

#[test]
fn f6_still_adds_a_connection_on_every_other_screen() {
    let bindings = KeyBindings::defaults();

    for scope in [Scope::Global, Scope::Connections, Scope::Transfers, Scope::History, Scope::Sync] {
        assert_eq!(bindings.map_key(scope, key(KeyCode::F(6))), Action::AddConnection, "{scope:?}");
    }
}

#[test]
fn m_does_nothing_outside_the_files_screen() {
    let bindings = KeyBindings::defaults();

    for scope in [Scope::Global, Scope::Connections, Scope::Transfers, Scope::History, Scope::Sync] {
        assert_eq!(bindings.map_key(scope, letter('m')), Action::Noop, "{scope:?}");
    }
}

#[test]
fn move_lists_the_letter_before_the_function_key() {
    let bindings = KeyBindings::defaults();
    let shown = |scope, action| bindings.keys_for(scope, action).into_iter().map(format_key_spec).collect::<Vec<_>>();

    assert_eq!(shown(Scope::Files, Action::Move), ["m", "F6"]);
}

#[test]
fn a_global_key_taken_by_a_screen_is_not_listed_for_the_global_action_there() {
    let bindings = KeyBindings::defaults();
    let shown = |scope, action| bindings.keys_for(scope, action).into_iter().map(format_key_spec).collect::<Vec<_>>();

    assert!(shown(Scope::Files, Action::AddConnection).is_empty());
    assert_eq!(shown(Scope::Connections, Action::AddConnection), ["n", "F6"]);
    assert_eq!(shown(Scope::Global, Action::AddConnection), ["F6"]);
    assert_eq!(shown(Scope::Files, Action::Quit), ["q", "F10"]);
}

#[test]
fn move_has_a_name_that_round_trips_and_is_in_the_action_list() {
    assert_eq!(Action::Move.name(), "move");
    assert_eq!(Action::from_name("move"), Some(Action::Move));
    assert!(ALL_ACTIONS.contains(&Action::Move));
}

#[test]
fn the_speed_limit_is_ctrl_l_on_the_transfers_screen_only() {
    let bindings = KeyBindings::defaults();

    assert_eq!(bindings.map_key(Scope::Transfers, ctrl('l')), Action::SpeedLimit);
    for scope in [Scope::Global, Scope::Files, Scope::Connections, Scope::History, Scope::Sync] {
        assert_eq!(bindings.map_key(scope, ctrl('l')), Action::Noop, "{scope:?}");
    }
}

#[test]
fn the_speed_limit_has_a_name_that_round_trips_and_is_in_the_action_list() {
    assert_eq!(Action::SpeedLimit.name(), "speed_limit");
    assert_eq!(Action::from_name("speed_limit"), Some(Action::SpeedLimit));
    assert!(ALL_ACTIONS.contains(&Action::SpeedLimit));
}

#[test]
fn the_speed_limit_key_is_listed_as_ctrl_l() {
    let bindings = KeyBindings::defaults();
    let shown = |scope, action| bindings.keys_for(scope, action).into_iter().map(format_key_spec).collect::<Vec<_>>();

    assert_eq!(shown(Scope::Transfers, Action::SpeedLimit), ["Ctrl+L"]);
}

#[test]
fn pause_is_p_on_the_transfers_screen_only() {
    let bindings = KeyBindings::defaults();

    assert_eq!(bindings.map_key(Scope::Transfers, letter('p')), Action::Pause);
    for scope in [Scope::Global, Scope::Connections, Scope::History, Scope::Sync] {
        assert_eq!(bindings.map_key(scope, letter('p')), Action::Noop, "{scope:?}");
    }
}

#[test]
fn pause_has_a_name_that_round_trips_and_is_in_the_action_list() {
    assert_eq!(Action::Pause.name(), "pause");
    assert_eq!(Action::from_name("pause"), Some(Action::Pause));
    assert!(ALL_ACTIONS.contains(&Action::Pause));
}

fn shifted(character: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(character), KeyModifiers::SHIFT)
}

#[test]
fn capital_k_and_j_reorder_on_the_transfers_screen_only() {
    let bindings = KeyBindings::defaults();

    assert_eq!(bindings.map_key(Scope::Transfers, shifted('K')), Action::RowUp);
    assert_eq!(bindings.map_key(Scope::Transfers, shifted('J')), Action::RowDown);
    assert_eq!(bindings.map_key(Scope::Transfers, letter('k')), Action::Up);
    assert_eq!(bindings.map_key(Scope::Transfers, letter('j')), Action::Down);
    for scope in [Scope::Global, Scope::Files, Scope::Connections, Scope::History, Scope::Sync] {
        assert_eq!(bindings.map_key(scope, shifted('K')), Action::Noop, "{scope:?}");
        assert_eq!(bindings.map_key(scope, shifted('J')), Action::Noop, "{scope:?}");
    }
}

#[test]
fn the_reorder_actions_have_names_that_round_trip_and_are_listed() {
    assert_eq!(Action::RowUp.name(), "row_up");
    assert_eq!(Action::RowDown.name(), "row_down");
    assert_eq!(Action::from_name("row_up"), Some(Action::RowUp));
    assert_eq!(Action::from_name("row_down"), Some(Action::RowDown));
    assert!(ALL_ACTIONS.contains(&Action::RowUp) && ALL_ACTIONS.contains(&Action::RowDown));
}

#[test]
fn the_reorder_keys_are_listed_as_capital_letters() {
    let bindings = KeyBindings::defaults();
    let shown = |scope, action| bindings.keys_for(scope, action).into_iter().map(format_key_spec).collect::<Vec<_>>();

    assert_eq!(shown(Scope::Transfers, Action::RowUp), ["K"]);
    assert_eq!(shown(Scope::Transfers, Action::RowDown), ["J"]);
}

#[test]
fn p_previews_on_the_files_screen_and_pauses_on_the_transfers_screen() {
    let bindings = KeyBindings::defaults();

    assert_eq!(bindings.map_key(Scope::Files, letter('p')), Action::Preview);
    assert_eq!(bindings.map_key(Scope::Transfers, letter('p')), Action::Pause);
    for scope in [Scope::Global, Scope::Connections, Scope::History, Scope::Sync] {
        assert_eq!(bindings.map_key(scope, letter('p')), Action::Noop, "{scope:?}");
    }
}

#[test]
fn preview_has_a_name_that_round_trips_and_is_listed_with_its_key() {
    let bindings = KeyBindings::defaults();
    let shown = |scope, action| bindings.keys_for(scope, action).into_iter().map(format_key_spec).collect::<Vec<_>>();

    assert_eq!(Action::Preview.name(), "preview");
    assert_eq!(Action::from_name("preview"), Some(Action::Preview));
    assert!(ALL_ACTIONS.contains(&Action::Preview));
    assert_eq!(shown(Scope::Files, Action::Preview), ["p"]);
}

#[test]
fn check_for_updates_round_trips_through_its_config_name() {
    assert_eq!(Action::CheckForUpdates.name(), "check_for_updates");
    assert_eq!(Action::from_name("check_for_updates"), Some(Action::CheckForUpdates));
    assert!(ALL_ACTIONS.contains(&Action::CheckForUpdates));
}

#[test]
fn check_for_updates_has_no_default_key() {
    for scope in [Scope::Global, Scope::Files, Scope::Connections, Scope::Transfers, Scope::History, Scope::Sync] {
        assert_eq!(KeyBindings::defaults().key_for(scope, Action::CheckForUpdates), None, "{scope:?}");
    }
}

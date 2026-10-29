use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

use super::{Action, Chord, KeyPress, Shortcuts, effective, is_reserved};

fn named(key: Named) -> Key {
    Key::Named(key)
}

fn character(text: &str) -> Key {
    Key::Character(text.into())
}

fn config(text: &str) -> toml::Table {
    text.parse().unwrap()
}

fn lookup(shortcuts: &Shortcuts, key: Key, modifiers: Modifiers) -> Option<Action> {
    shortcuts.action_for(&key, modifiers)
}

#[test]
fn the_default_table_maps_arrows_and_vim_keys() {
    let shortcuts = Shortcuts::default();
    let none = Modifiers::empty();

    assert_eq!(lookup(&shortcuts, named(Named::ArrowUp), none), Some(Action::MoveUp));
    assert_eq!(lookup(&shortcuts, character("k"), none), Some(Action::MoveUp));
    assert_eq!(lookup(&shortcuts, named(Named::ArrowDown), none), Some(Action::MoveDown));
    assert_eq!(lookup(&shortcuts, character("j"), none), Some(Action::MoveDown));
    assert_eq!(lookup(&shortcuts, named(Named::PageUp), none), Some(Action::PageUp));
    assert_eq!(lookup(&shortcuts, named(Named::PageDown), none), Some(Action::PageDown));
    assert_eq!(lookup(&shortcuts, named(Named::Home), none), Some(Action::Home));
    assert_eq!(lookup(&shortcuts, named(Named::End), none), Some(Action::End));
    assert_eq!(lookup(&shortcuts, named(Named::Enter), none), Some(Action::Open));
    assert_eq!(lookup(&shortcuts, named(Named::Tab), none), Some(Action::SwitchPanel));
    assert_eq!(lookup(&shortcuts, named(Named::Space), none), Some(Action::ToggleSelect));
    assert_eq!(lookup(&shortcuts, character("/"), none), Some(Action::StartFilter));
    assert_eq!(lookup(&shortcuts, named(Named::Escape), none), Some(Action::ClearFilter));
}

#[test]
fn ctrl_h_toggles_hidden_and_backspace_goes_to_the_parent() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, character("h"), Modifiers::CTRL), Some(Action::ToggleHidden));
    assert_eq!(lookup(&shortcuts, named(Named::Backspace), Modifiers::empty()), Some(Action::Parent));
    assert_eq!(lookup(&shortcuts, character("s"), Modifiers::CTRL), Some(Action::CycleSort));
    assert_eq!(lookup(&shortcuts, character("r"), Modifiers::CTRL), Some(Action::Refresh));
    assert_eq!(lookup(&shortcuts, character("n"), Modifiers::CTRL), Some(Action::NextTab));
}

#[test]
fn the_platform_command_key_selects_all_and_quits() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, character("a"), Modifiers::COMMAND), Some(Action::SelectAll));
    assert_eq!(lookup(&shortcuts, character("q"), Modifiers::COMMAND), Some(Action::Quit));
}

#[test]
fn modifiers_must_match_exactly() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, character("j"), Modifiers::CTRL), None);
    assert_eq!(lookup(&shortcuts, character("h"), Modifiers::empty()), None);
    assert_eq!(lookup(&shortcuts, character("h"), Modifiers::CTRL | Modifiers::ALT), None);
    assert_eq!(lookup(&shortcuts, character("J"), Modifiers::empty()), Some(Action::MoveDown));
}

#[test]
fn no_default_binding_uses_a_reserved_chord() {
    for (chord, action) in Shortcuts::default().bindings() {
        assert!(!is_reserved(chord), "{action:?} is bound to a reserved chord {chord:?}");
    }
}

#[test]
fn the_window_manager_chords_are_reserved() {
    let reserved = |key: Key, modifiers: Modifiers| is_reserved(&Chord::new(key, modifiers));

    assert!(reserved(named(Named::F4), Modifiers::ALT));
    assert!(reserved(named(Named::F11), Modifiers::empty()));
    assert!(reserved(named(Named::Tab), Modifiers::ALT));
    assert!(reserved(character("t"), Modifiers::CTRL | Modifiers::SHIFT));
    assert!(!reserved(character("t"), Modifiers::CTRL));
    assert!(!reserved(named(Named::F5), Modifiers::empty()));
}

#[test]
fn a_configured_chord_replaces_a_default() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\ncycle_sort = \"Ctrl+H\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, character("h"), Modifiers::CTRL), Some(Action::CycleSort));
    assert_eq!(lookup(&shortcuts, character("s"), Modifiers::CTRL), None);
}

#[test]
fn a_list_of_chords_binds_every_one_of_them() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\nmove_up = [\"w\", \"Alt+Up\"]\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, character("w"), Modifiers::empty()), Some(Action::MoveUp));
    assert_eq!(lookup(&shortcuts, named(Named::ArrowUp), Modifiers::ALT), Some(Action::MoveUp));
    assert_eq!(lookup(&shortcuts, named(Named::ArrowUp), Modifiers::empty()), None);
}

#[test]
fn chord_text_understands_names_modifiers_and_case() {
    let text = "[gui.keys]\nopen = [\"F5\", \"shift+tab\", \"MOD+K\", \"Ctrl++\", \"Alt+PageDown\"]\n";
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config(text));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, named(Named::F5), Modifiers::empty()), Some(Action::Open));
    assert_eq!(lookup(&shortcuts, named(Named::Tab), Modifiers::SHIFT), Some(Action::Open));
    assert_eq!(lookup(&shortcuts, character("k"), Modifiers::COMMAND), Some(Action::Open));
    assert_eq!(lookup(&shortcuts, character("+"), Modifiers::CTRL), Some(Action::Open));
    assert_eq!(lookup(&shortcuts, named(Named::PageDown), Modifiers::ALT), Some(Action::Open));
}

#[test]
fn an_unknown_action_or_chord_in_the_config_warns() {
    let text = "[gui.keys]\nbogus = \"x\"\nmove_up = \"Hyper+Z\"\nmove_down = 5\npage_up = \"Alt+F4\"\nhome = \"\"\n";
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config(text));

    assert_eq!(warnings.len(), 5, "{warnings:?}");
    assert!(warnings.iter().any(|warning| warning.contains("unknown gui key action \"bogus\"")), "{warnings:?}");
    assert!(warnings.iter().any(|warning| warning.contains("Hyper+Z")), "{warnings:?}");
    assert!(warnings.iter().any(|warning| warning.contains("move_down")), "{warnings:?}");
    assert!(warnings.iter().any(|warning| warning.contains("reserved")), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, named(Named::ArrowUp), Modifiers::empty()), Some(Action::MoveUp));
    assert_eq!(lookup(&shortcuts, named(Named::PageUp), Modifiers::empty()), Some(Action::PageUp));
    assert_eq!(lookup(&shortcuts, named(Named::Home), Modifiers::empty()), Some(Action::Home));
}

#[test]
fn a_missing_or_malformed_keys_table_keeps_the_defaults() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config(""));
    assert!(warnings.is_empty());
    assert_eq!(lookup(&shortcuts, character("k"), Modifiers::empty()), Some(Action::MoveUp));

    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui]\nkeys = 3\n"));
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(lookup(&shortcuts, character("k"), Modifiers::empty()), Some(Action::MoveUp));
}

#[test]
fn a_shifted_punctuation_key_is_read_as_the_character_it_types() {
    let german_slash =
        KeyPress { key: character("7"), modified_key: character("/"), modifiers: Modifiers::SHIFT, text: None };
    assert_eq!(effective(&german_slash), (character("/"), Modifiers::empty()));

    let plain =
        KeyPress { key: character("/"), modified_key: character("/"), modifiers: Modifiers::empty(), text: None };
    assert_eq!(effective(&plain), (character("/"), Modifiers::empty()));

    let shifted_letter =
        KeyPress { key: character("k"), modified_key: character("K"), modifiers: Modifiers::SHIFT, text: None };
    assert_eq!(effective(&shifted_letter), (character("k"), Modifiers::SHIFT));

    let ctrl_digit =
        KeyPress { key: character("7"), modified_key: character("/"), modifiers: Modifiers::CTRL, text: None };
    assert_eq!(effective(&ctrl_digit), (character("7"), Modifiers::CTRL));
}

#[test]
fn the_file_action_keys_are_bound() {
    let shortcuts = Shortcuts::default();
    let none = Modifiers::empty();

    assert_eq!(lookup(&shortcuts, named(Named::F7), none), Some(Action::NewFolder));
    assert_eq!(lookup(&shortcuts, named(Named::F2), none), Some(Action::Rename));
    assert_eq!(lookup(&shortcuts, named(Named::Delete), none), Some(Action::Delete));
    assert_eq!(lookup(&shortcuts, named(Named::F8), none), Some(Action::Delete));
}

#[test]
fn the_transfer_keys_are_bound() {
    let shortcuts = Shortcuts::default();
    let none = Modifiers::empty();

    assert_eq!(lookup(&shortcuts, named(Named::F5), none), Some(Action::Copy));
    assert_eq!(lookup(&shortcuts, named(Named::F6), none), Some(Action::Move));
}

#[test]
fn control_t_toggles_the_queue() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, character("t"), Modifiers::CTRL), Some(Action::ToggleQueue));
}

#[test]
fn label_for_shows_the_first_binding() {
    let shortcuts = Shortcuts::default();
    let command = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };

    assert_eq!(shortcuts.label_for(Action::Copy), Some("F5".to_string()));
    assert_eq!(shortcuts.label_for(Action::ToggleQueue), Some("Ctrl+T".to_string()));
    assert_eq!(shortcuts.label_for(Action::Delete), Some("Delete".to_string()));
    assert_eq!(shortcuts.label_for(Action::Quit), Some(format!("{command}+Q")));
}

#[test]
fn label_for_follows_a_rebinding() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\ncopy = \"F9\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(shortcuts.label_for(Action::Copy), Some("F9".to_string()));
}

#[test]
fn label_for_an_unbound_action_is_none() {
    let shortcuts = Shortcuts { bindings: Vec::new() };

    assert_eq!(shortcuts.label_for(Action::Copy), None);
}

#[test]
fn label_for_spells_modifier_combinations_in_a_fixed_order() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\nrename = \"ctrl+alt+x\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(shortcuts.label_for(Action::Rename), Some("Ctrl+Alt+X".to_string()));
}

#[test]
fn control_f_searches() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, character("f"), Modifiers::CTRL), Some(Action::Search));
}

#[test]
fn search_round_trips_by_name() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\nsearch = \"Ctrl+G\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, character("g"), Modifiers::CTRL), Some(Action::Search));
}

#[test]
fn p_previews() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, character("p"), Modifiers::empty()), Some(Action::Preview));
}

#[test]
fn preview_round_trips_by_name() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\npreview = \"F9\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, named(Named::F9), Modifiers::empty()), Some(Action::Preview));
}

#[test]
fn e_and_f3_edit() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, character("e"), Modifiers::empty()), Some(Action::Edit));
    assert_eq!(lookup(&shortcuts, named(Named::F3), Modifiers::empty()), Some(Action::Edit));
}

#[test]
fn edit_can_be_rebound_by_name() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\nedit = \"F9\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, named(Named::F9), Modifiers::empty()), Some(Action::Edit));
    assert_eq!(lookup(&shortcuts, character("e"), Modifiers::empty()), None);
}

#[test]
fn f4_opens_a_terminal() {
    let shortcuts = Shortcuts::default();

    assert_eq!(lookup(&shortcuts, named(Named::F4), Modifiers::empty()), Some(Action::Terminal));
}

#[test]
fn terminal_can_be_rebound_by_name() {
    let (shortcuts, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\nterminal = \"F9\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&shortcuts, named(Named::F9), Modifiers::empty()), Some(Action::Terminal));
}

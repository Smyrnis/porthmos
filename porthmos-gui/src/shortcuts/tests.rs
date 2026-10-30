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

#[test]
fn ctrl_u_and_the_sync_name_round_trip() {
    let shortcuts = Shortcuts::default();
    assert_eq!(lookup(&shortcuts, character("u"), Modifiers::CTRL), Some(Action::Sync));

    let (rebound, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\nsync = \"F9\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&rebound, named(Named::F9), Modifiers::empty()), Some(Action::Sync));
    assert_eq!(lookup(&rebound, character("u"), Modifiers::CTRL), None);
}

#[test]
fn check_for_updates_can_be_rebound_by_name() {
    let defaults = Shortcuts::default();

    assert_eq!(defaults.label_for(Action::CheckForUpdates), None);

    let (rebound, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\ncheck_for_updates = \"f9\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&rebound, named(Named::F9), Modifiers::empty()), Some(Action::CheckForUpdates));
    assert_eq!(rebound.label_for(Action::CheckForUpdates).as_deref(), Some("F9"));
}

#[test]
fn ctrl_p_and_the_palette_name_round_trip() {
    let defaults = Shortcuts::default();

    assert_eq!(lookup(&defaults, character("p"), Modifiers::CTRL), Some(Action::Palette));
    assert_eq!(lookup(&defaults, character("p"), Modifiers::empty()), Some(Action::Preview));
    assert_eq!(defaults.label_for(Action::Palette).as_deref(), Some("Ctrl+P"));

    let (rebound, warnings) = Shortcuts::from_frontend(&config("[gui.keys]\npalette = \"ctrl+o\"\n"));

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(lookup(&rebound, character("o"), Modifiers::CTRL), Some(Action::Palette));
    assert_eq!(lookup(&rebound, character("p"), Modifiers::CTRL), None);
}

fn chord(key: Key, modifiers: Modifiers) -> Chord {
    Chord::new(key, modifiers)
}

fn press_of(key: Key, modified_key: Key, modifiers: Modifiers) -> KeyPress {
    KeyPress { key, modified_key, modifiers, text: None }
}

fn rank(action: Action) -> usize {
    match action {
        Action::MoveUp => 0,
        Action::MoveDown => 1,
        Action::PageUp => 2,
        Action::PageDown => 3,
        Action::Home => 4,
        Action::End => 5,
        Action::Open => 6,
        Action::Parent => 7,
        Action::SwitchPanel => 8,
        Action::ToggleSelect => 9,
        Action::SelectAll => 10,
        Action::ToggleHidden => 11,
        Action::CycleSort => 12,
        Action::Refresh => 13,
        Action::StartFilter => 14,
        Action::ClearFilter => 15,
        Action::NextTab => 16,
        Action::PreviousTab => 17,
        Action::NewFolder => 18,
        Action::Rename => 19,
        Action::Delete => 20,
        Action::Copy => 21,
        Action::Move => 22,
        Action::ToggleQueue => 23,
        Action::Preview => 24,
        Action::Edit => 25,
        Action::Terminal => 26,
        Action::Search => 27,
        Action::Bookmarks => 28,
        Action::BookmarkHere => 29,
        Action::History => 30,
        Action::Sync => 31,
        Action::Palette => 32,
        Action::Settings => 33,
        Action::CheckForUpdates => 34,
        Action::Quit => 35,
    }
}

#[test]
fn every_action_has_a_unique_name_and_a_non_empty_title() {
    let names: Vec<&str> = Action::all().map(Action::name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();

    assert_eq!(sorted.len(), names.len(), "{names:?}");
    for action in Action::all() {
        assert!(!action.title().trim().is_empty(), "{action:?}");
        assert_eq!(Action::from_name(action.name()), Some(action));
    }
    let mut titles: Vec<&str> = Action::all().map(Action::title).collect();
    titles.sort_unstable();
    titles.dedup();
    assert_eq!(titles.len(), names.len(), "two actions share a title");
}

#[test]
fn all_lists_every_action_once_in_names_order() {
    let ranks: Vec<usize> = Action::all().map(rank).collect();

    assert_eq!(ranks, (0..ranks.len()).collect::<Vec<_>>());
    assert_eq!(ranks.len(), rank(Action::Quit) + 1);
}

fn every_modifier_combination() -> Vec<Modifiers> {
    (0..16)
        .map(|bits| {
            let mut modifiers = Modifiers::empty();
            for (bit, flag) in
                [Modifiers::CTRL, Modifiers::ALT, Modifiers::SHIFT, Modifiers::LOGO].into_iter().enumerate()
            {
                if bits & (1 << bit) != 0 {
                    modifiers |= flag;
                }
            }
            modifiers
        })
        .collect()
}

#[test]
fn a_chord_spec_parses_back_to_the_same_chord() {
    let mut keys: Vec<Key> = super::NAMED_KEYS.iter().map(|(_, named)| Key::Named(*named)).collect();
    keys.extend(('a'..='z').chain('0'..='9').map(|letter| character(&letter.to_string())));
    keys.extend(["+", "-", "/", ",", ".", ";", "'", "[", "]", "\\", "=", "`", "é", "ß"].map(character));

    for key in keys {
        for modifiers in every_modifier_combination() {
            let original = chord(key.clone(), modifiers);
            let spec = original.spec();

            assert_eq!(Chord::parse(&spec), Some(original), "{spec}");
        }
    }
}

#[test]
fn spec_orders_modifiers_the_same_way_every_time() {
    let everything = Modifiers::SHIFT | Modifiers::LOGO | Modifiers::ALT | Modifiers::CTRL;

    assert_eq!(chord(character("A"), everything).spec(), "ctrl+alt+shift+cmd+a");
    assert_eq!(chord(named(Named::Enter), Modifiers::empty()).spec(), "enter");
    assert_eq!(chord(named(Named::PageUp), Modifiers::ALT).spec(), "alt+pageup");
    assert_eq!(chord(character("+"), Modifiers::CTRL).spec(), "ctrl++");
}

#[test]
fn chords_for_lists_the_bindings_of_an_action() {
    let shortcuts = Shortcuts::default();

    assert_eq!(
        shortcuts.chords_for(Action::MoveUp),
        [chord(named(Named::ArrowUp), Modifiers::empty()), chord(character("k"), Modifiers::empty())]
    );
    assert_eq!(shortcuts.chords_for(Action::Palette), [chord(character("p"), Modifiers::CTRL)]);
    assert!(shortcuts.chords_for(Action::CheckForUpdates).is_empty());
}

#[test]
fn default_chords_ignore_user_changes() {
    let (rebound, _) = Shortcuts::from_frontend(&config("[gui.keys]\nmove_up = \"w\"\n"));

    assert_eq!(rebound.chords_for(Action::MoveUp), [chord(character("w"), Modifiers::empty())]);
    assert_eq!(Shortcuts::default_chords(Action::MoveUp), Shortcuts::default().chords_for(Action::MoveUp));
}

#[test]
fn owner_of_finds_the_action_or_none() {
    let shortcuts = Shortcuts::default();

    assert_eq!(shortcuts.owner_of(&chord(character("F"), Modifiers::CTRL)), Some(Action::Search));
    assert_eq!(shortcuts.owner_of(&chord(character("f"), Modifiers::ALT)), None);
}

#[test]
fn assign_replaces_all_chords_of_the_action() {
    let mut shortcuts = Shortcuts::default();
    let wanted = chord(character("w"), Modifiers::empty());

    assert_eq!(shortcuts.assign(Action::MoveUp, wanted.clone()), Ok(()));

    assert_eq!(shortcuts.chords_for(Action::MoveUp), [wanted]);
    assert_eq!(lookup(&shortcuts, named(Named::ArrowUp), Modifiers::empty()), None);
    assert_eq!(lookup(&shortcuts, character("k"), Modifiers::empty()), None);
    assert_eq!(lookup(&shortcuts, character("w"), Modifiers::empty()), Some(Action::MoveUp));
}

#[test]
fn assign_refuses_a_reserved_chord() {
    let mut shortcuts = Shortcuts::default();
    let reserved = [
        chord(named(Named::F4), Modifiers::ALT),
        chord(named(Named::F11), Modifiers::empty()),
        chord(character("x"), Modifiers::CTRL | Modifiers::SHIFT),
    ];

    for refused in reserved {
        assert_eq!(shortcuts.assign(Action::Refresh, refused), Err(super::AssignError::Reserved));
    }
    assert_eq!(shortcuts.chords_for(Action::Refresh), Shortcuts::default_chords(Action::Refresh));
}

#[test]
fn assign_refuses_a_chord_another_action_uses_and_names_it() {
    let mut shortcuts = Shortcuts::default();

    let outcome = shortcuts.assign(Action::Refresh, chord(character("f"), Modifiers::CTRL));

    assert_eq!(outcome, Err(super::AssignError::UsedBy(Action::Search)));
    assert_eq!(shortcuts.chords_for(Action::Refresh), Shortcuts::default_chords(Action::Refresh));
    assert_eq!(shortcuts.chords_for(Action::Search), Shortcuts::default_chords(Action::Search));
}

#[test]
fn assign_the_chord_the_action_already_has_succeeds() {
    let mut shortcuts = Shortcuts::default();
    let own = chord(character("k"), Modifiers::empty());

    assert_eq!(shortcuts.assign(Action::MoveUp, own.clone()), Ok(()));

    assert_eq!(shortcuts.chords_for(Action::MoveUp), [own]);
}

#[test]
fn is_modified_follows_assign_and_reset() {
    let mut shortcuts = Shortcuts::default();
    assert!(!shortcuts.is_modified(Action::Refresh));

    shortcuts.assign(Action::Refresh, chord(character("o"), Modifiers::CTRL)).unwrap();
    assert!(shortcuts.is_modified(Action::Refresh));
    assert!(!shortcuts.is_modified(Action::Search));

    shortcuts.reset(Action::Refresh);
    assert!(!shortcuts.is_modified(Action::Refresh));
}

#[test]
fn reset_restores_the_defaults() {
    let mut shortcuts = Shortcuts::default();
    shortcuts.assign(Action::MoveUp, chord(character("w"), Modifiers::empty())).unwrap();

    shortcuts.reset(Action::MoveUp);

    assert_eq!(shortcuts.chords_for(Action::MoveUp), Shortcuts::default_chords(Action::MoveUp));
    assert_eq!(lookup(&shortcuts, character("w"), Modifiers::empty()), None);
    assert_eq!(lookup(&shortcuts, character("k"), Modifiers::empty()), Some(Action::MoveUp));
}

#[test]
fn reset_skips_a_default_chord_another_action_took() {
    let mut shortcuts = Shortcuts::default();
    shortcuts.assign(Action::MoveUp, chord(character("w"), Modifiers::empty())).unwrap();
    shortcuts.assign(Action::Copy, chord(character("k"), Modifiers::empty())).unwrap();

    shortcuts.reset(Action::MoveUp);

    assert_eq!(shortcuts.chords_for(Action::MoveUp), [chord(named(Named::ArrowUp), Modifiers::empty())]);
    assert_eq!(lookup(&shortcuts, character("k"), Modifiers::empty()), Some(Action::Copy));
}

#[test]
fn chord_from_press_reads_the_effective_key_and_modifiers() {
    let german_slash = press_of(character("7"), character("/"), Modifiers::SHIFT);
    assert_eq!(super::chord_from_press(&german_slash), Some(chord(character("/"), Modifiers::empty())));

    let ctrl_k = press_of(character("k"), character("k"), Modifiers::CTRL);
    assert_eq!(super::chord_from_press(&ctrl_k), Some(chord(character("k"), Modifiers::CTRL)));

    let shifted_letter = press_of(character("k"), character("K"), Modifiers::SHIFT);
    assert_eq!(super::chord_from_press(&shifted_letter), Some(chord(character("k"), Modifiers::SHIFT)));

    let function = press_of(named(Named::F9), named(Named::F9), Modifiers::ALT);
    assert_eq!(super::chord_from_press(&function), Some(chord(named(Named::F9), Modifiers::ALT)));
}

#[test]
fn chord_from_press_ignores_a_modifier_only_press() {
    for modifier in
        [Named::Shift, Named::Control, Named::Alt, Named::Super, Named::Meta, Named::AltGraph, Named::CapsLock]
    {
        let held = press_of(named(modifier), named(modifier), Modifiers::SHIFT);

        assert_eq!(super::chord_from_press(&held), None, "{modifier:?}");
    }
}

#[test]
fn chord_from_press_ignores_keys_the_config_cannot_name() {
    let unnamed = [named(Named::F13), named(Named::MediaPlayPause), Key::Unidentified, character(" "), character("")];

    for key in unnamed {
        assert_eq!(super::chord_from_press(&press_of(key.clone(), key.clone(), Modifiers::empty())), None, "{key:?}");
    }
}

#[test]
fn assigned_chords_survive_a_config_round_trip() {
    let assigned = [
        (Action::Refresh, chord(character("o"), Modifiers::CTRL)),
        (Action::Copy, chord(named(Named::F9), Modifiers::ALT)),
        (Action::Rename, chord(character("+"), Modifiers::CTRL)),
        (Action::Delete, chord(character("\\"), Modifiers::ALT)),
        (Action::Preview, chord(character("\""), Modifiers::empty())),
        (Action::Edit, chord(character("é"), Modifiers::CTRL)),
        (Action::Sync, chord(named(Named::PageDown), Modifiers::CTRL | Modifiers::ALT)),
    ];
    let mut shortcuts = Shortcuts::default();
    let mut keys = toml::Table::new();
    for (action, wanted) in &assigned {
        shortcuts.assign(*action, wanted.clone()).unwrap();
        keys.insert(action.name().to_string(), toml::Value::String(wanted.spec()));
    }
    let mut gui = toml::Table::new();
    gui.insert("keys".to_string(), toml::Value::Table(keys));
    let mut frontend = toml::Table::new();
    frontend.insert("gui".to_string(), toml::Value::Table(gui));

    let (loaded, warnings) = Shortcuts::from_frontend(&frontend);

    assert!(warnings.is_empty(), "{warnings:?}");
    for action in Action::all() {
        assert_eq!(loaded.chords_for(action), shortcuts.chords_for(action), "{action:?}");
    }
}

#[test]
fn labels_spell_the_page_keys_in_camel_case() {
    assert_eq!(chord(named(Named::PageUp), Modifiers::empty()).label(), "PageUp");
    assert_eq!(chord(named(Named::PageDown), Modifiers::ALT).label(), "Alt+PageDown");
    assert_eq!(chord(named(Named::Escape), Modifiers::empty()).label(), "Escape");
    assert_eq!(chord(named(Named::F5), Modifiers::empty()).label(), "F5");
}

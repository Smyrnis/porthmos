use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use porthmos_core::{
    Command, ConnectionForm, Event, Location, ProtocolInfo, Severity,
    profiles::{ConnectionEntry, ConnectionSource},
};

use super::*;
use crate::{
    app::testing::{
        TYPED, TestApp, cell_fg, entry, find_text, has_unprintable_cell, render_app, screen_rows, test_app,
        test_app_with_protocols,
    },
    input::KeyBindings,
    palette::{Entry as PaletteEntry, Group, Target},
    widgets::filter_line::FilterLine,
};

const SCREENS: [Screen; 6] =
    [Screen::Files, Screen::Connections, Screen::Search, Screen::Transfers, Screen::History, Screen::Sync];

fn connection(name: &str, protocol: &str, source: ConnectionSource) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: protocol.to_string(),
        host: format!("{name}.example.com"),
        port: 22,
        username: "deploy".to_string(),
        password: None,
        options: Default::default(),
        source,
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
    }
}

fn protocols() -> Vec<ProtocolInfo> {
    vec![
        ProtocolInfo {
            id: "sftp",
            display_name: "SFTP",
            description: "",
            limits_note: None,
            form: ConnectionForm::standard(22),
        },
        ProtocolInfo {
            id: "ftp",
            display_name: "FTP",
            description: "",
            limits_note: None,
            form: ConnectionForm::standard(21),
        },
    ]
}

fn on(screen: Screen) -> TestApp {
    let mut test = test_app_with_protocols(Path::new("/d"), protocols());
    test.app.screen = screen;
    test
}

fn labels(entries: &[PaletteEntry], group: Group) -> Vec<&str> {
    entries.iter().filter(|entry| entry.group == group).map(|entry| entry.label.as_str()).collect()
}

fn find<'a>(entries: &'a [PaletteEntry], label: &str) -> &'a PaletteEntry {
    entries.iter().find(|entry| entry.label == label).unwrap_or_else(|| panic!("{label} is not listed"))
}

#[test]
fn actions_follow_the_current_screen() {
    let files = on(Screen::Files).app.palette_entries();
    let files = labels(&files, Group::Actions);
    assert!(files.contains(&"Copy to the other panel") && files.contains(&"New folder"));
    assert!(!files.contains(&"Delete connection") && !files.contains(&"Retry the selected transfer"));

    let connections = on(Screen::Connections).app.palette_entries();
    let connections = labels(&connections, Group::Actions);
    assert!(connections.contains(&"Delete connection") && connections.contains(&"Edit connection"));
    assert!(!connections.contains(&"Copy to the other panel"));

    let transfers = on(Screen::Transfers).app.palette_entries();
    let transfers = labels(&transfers, Group::Actions);
    for label in ["Retry the selected transfer", "Cancel the selected transfer", "Clear finished transfers"] {
        assert!(transfers.contains(&label), "{label}");
    }

    let history = on(Screen::History).app.palette_entries();
    let history = labels(&history, Group::Actions);
    assert!(history.contains(&"Clear the history") && history.contains(&"Show the details"));

    let sync = on(Screen::Sync).app.palette_entries();
    let sync = labels(&sync, Group::Actions);
    assert!(sync.contains(&"Filter the preview") && !sync.contains(&"Copy to the other panel"));
}

#[test]
fn some_actions_are_offered_everywhere() {
    for screen in SCREENS {
        let entries = on(screen).app.palette_entries();
        let actions = labels(&entries, Group::Actions);
        for label in
            ["Quit", "Help", "Connections", "New connection", "Transfers", "Transfer history", "Cancel all transfers"]
        {
            assert!(actions.contains(&label), "{screen:?} {label}");
        }
    }
}

#[test]
fn never_listed_actions_are_absent_everywhere() {
    for screen in SCREENS {
        for entry in on(screen).app.palette_entries() {
            if let Target::Action(action) = entry.target {
                assert!(
                    !matches!(
                        action,
                        Action::Up
                            | Action::Down
                            | Action::ToggleSelect
                            | Action::Back
                            | Action::Disconnect
                            | Action::JumpToSession(_)
                            | Action::OpenPalette
                            | Action::Noop
                    ),
                    "{screen:?} {action:?}"
                );
            }
        }
    }
}

#[test]
fn every_label_is_non_empty_and_unique_within_a_screen() {
    for screen in SCREENS {
        let entries = on(screen).app.palette_entries();
        assert!(entries.iter().all(|entry| !entry.label.trim().is_empty()), "{screen:?}");
        let actions = labels(&entries, Group::Actions);
        let mut unique = actions.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), actions.len(), "{screen:?}: {actions:?}");
    }
}

#[test]
fn an_action_hint_is_its_first_live_key_in_the_current_scope() {
    let files = on(Screen::Files).app.palette_entries();
    assert_eq!(find(&files, "Copy to the other panel").hint.as_deref(), Some("c"));
    assert_eq!(find(&files, "Quit").hint.as_deref(), Some("q"));
    let connections = on(Screen::Connections).app.palette_entries();
    assert_eq!(find(&connections, "New connection").hint.as_deref(), Some("n"));
    assert_eq!(find(&connections, "Edit connection").hint.as_deref(), Some("e"));
    let transfers = on(Screen::Transfers).app.palette_entries();
    assert_eq!(find(&transfers, "Clear finished transfers").hint.as_deref(), Some("c"));
}

#[test]
fn a_remapped_key_shows_and_an_unbound_action_has_no_hint() {
    let mut test = on(Screen::Files);
    let overrides = std::collections::HashMap::from([("copy".to_string(), "ctrl+k".to_string())]);
    test.app.key_bindings = KeyBindings::from_overrides(&overrides).0;
    assert_eq!(find(&test.app.palette_entries(), "Copy to the other panel").hint.as_deref(), Some("Ctrl+K"));

    let table: toml::Table = toml::from_str("[keys]\ncopy = []\n").unwrap();
    test.app.key_bindings = KeyBindings::from_frontend(&table).0;
    assert_eq!(find(&test.app.palette_entries(), "Copy to the other panel").hint, None);
}

#[test]
fn a_connect_entry_exists_for_each_saved_connection_with_its_protocol_as_the_hint() {
    let mut test = on(Screen::Files);
    test.app.connections.replace(vec![
        connection("prod", "sftp", ConnectionSource::Profile),
        connection("legacy", "ftp", ConnectionSource::Profile),
        connection("odd", "carrier-pigeon", ConnectionSource::Profile),
    ]);

    let entries = test.app.palette_entries();

    let prod = find(&entries, "Connect prod");
    assert_eq!((prod.group, prod.hint.as_deref()), (Group::Hosts, Some("SFTP")));
    assert_eq!(prod.target, Target::Connect("prod".to_string()));
    assert_eq!(find(&entries, "Connect legacy").hint.as_deref(), Some("FTP"));
    assert_eq!(find(&entries, "Connect odd").hint.as_deref(), Some("carrier-pigeon"));
}

#[test]
fn ssh_config_connections_are_included_and_label_only_orphans_are_not() {
    let mut test = on(Screen::Files);
    test.app.connections.replace(vec![
        connection("web", "sftp", ConnectionSource::SshConfig),
        connection("gone", "sftp", ConnectionSource::MissingSshHost),
        connection("hidden", "sftp", ConnectionSource::ShadowedSshHost),
    ]);

    let entries = test.app.palette_entries();
    let hosts = labels(&entries, Group::Hosts);

    assert_eq!(hosts, ["Connect web"]);
}

#[test]
fn a_disconnect_entry_exists_only_for_a_connected_session() {
    let mut test = on(Screen::Files);
    test.app.connections.replace(vec![
        connection("a", "sftp", ConnectionSource::Profile),
        connection("b", "sftp", ConnectionSource::Profile),
    ]);
    test.connect(1, "a");

    let entries = test.app.palette_entries();

    let disconnect = find(&entries, "Disconnect a");
    assert_eq!((disconnect.group, disconnect.hint.as_deref()), (Group::Hosts, Some("connected")));
    assert_eq!(disconnect.target, Target::Disconnect("a".to_string()));
    assert!(!entries.iter().any(|entry| entry.label == "Disconnect b"));
}

#[test]
fn file_entries_come_from_the_local_panel_and_the_active_remote_panel_with_a_slash_on_directories() {
    let mut test = on(Screen::Files);
    test.list_local(vec![entry(Path::new("/d"), "a.txt", false), entry(Path::new("/d"), "docs", true)]);
    test.connect(1, "prod");
    test.list_remote(1, "/srv", vec![entry(Path::new("/srv"), "index.html", false)]);

    let entries = test.app.palette_entries();

    assert_eq!(labels(&entries, Group::Files), ["docs/", "a.txt", "index.html"]);
    assert_eq!(find(&entries, "a.txt").hint.as_deref(), Some("local"));
    assert_eq!(find(&entries, "index.html").hint.as_deref(), Some("prod"));
    assert_eq!(find(&entries, "docs/").target, Target::File { location: Location::Local, name: "docs".to_string() });
    assert_eq!(
        find(&entries, "index.html").target,
        Target::File { location: Location::Session(1), name: "index.html".to_string() }
    );
}

#[test]
fn only_the_active_session_contributes_files() {
    let mut test = on(Screen::Files);
    test.connect(1, "a");
    test.list_remote(1, "/srv", vec![entry(Path::new("/srv"), "from_a", false)]);
    test.connect(2, "b");
    test.list_remote(2, "/srv", vec![entry(Path::new("/srv"), "from_b", false)]);

    let entries = test.app.palette_entries();

    assert_eq!(labels(&entries, Group::Files), ["from_b"]);
}

#[test]
fn the_parent_row_is_not_an_entry() {
    let mut test = on(Screen::Files);
    test.list_local(vec![entry(Path::new("/d"), "a.txt", false)]);

    let entries = test.app.palette_entries();

    assert!(!labels(&entries, Group::Files).iter().any(|label| label.starts_with("..")));
}

#[test]
fn hostile_names_are_sanitised_in_labels_and_hints_but_targets_keep_the_real_name() {
    let mut test = on(Screen::Files);
    let hostile = "a\u{1b}[2J\u{202e}b\nc";
    test.list_local(vec![entry(Path::new("/d"), hostile, false)]);
    test.app.connections.replace(vec![connection(hostile, "sftp", ConnectionSource::Profile)]);
    test.connect(1, hostile);
    test.list_remote(1, "/srv", vec![entry(Path::new("/srv"), &"x".repeat(500), false)]);

    let entries = test.app.palette_entries();

    for entry in &entries {
        let text = format!("{}{}", entry.label, entry.hint.as_deref().unwrap_or(""));
        assert!(
            !text.chars().any(|character| character.is_control() || ('\u{202a}'..='\u{202e}').contains(&character)),
            "{text:?}"
        );
    }
    assert!(entries.iter().any(|entry| entry.target == Target::Connect(hostile.to_string())));
    assert!(entries.iter().any(|entry| entry.target == Target::Disconnect(hostile.to_string())));
    assert!(
        entries
            .iter()
            .any(|entry| entry.target == Target::File { location: Location::Local, name: hostile.to_string() })
    );
    assert!(entries.iter().any(|entry| entry.label.len() == 500));
}

#[test]
fn entries_are_empty_for_missing_panels_without_panicking() {
    let test = test_app(Path::new("/d"));

    let entries = test.app.palette_entries();

    assert!(labels(&entries, Group::Files).is_empty());
    assert!(labels(&entries, Group::Hosts).is_empty());
    assert!(!labels(&entries, Group::Actions).is_empty());
}

#[test]
fn a_remote_session_that_has_not_listed_yet_adds_no_files() {
    let mut test = on(Screen::Files);
    test.connect(1, "prod");

    assert!(labels(&test.app.palette_entries(), Group::Files).is_empty());
}

fn press(test: &mut TestApp, code: KeyCode) {
    test.app.apply_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn ctrl_p(test: &mut TestApp) {
    test.app.apply_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
}

fn open_with(test: &mut TestApp, query: &str) {
    ctrl_p(test);
    assert!(test.app.palette.is_some(), "the palette did not open");
    test.type_text(query);
}

fn palette_query(test: &TestApp) -> String {
    test.app.palette.as_ref().map(|palette| palette.query().to_string()).unwrap_or_default()
}

fn screen_of(screen: Screen) -> TestApp {
    let mut test = on(screen);
    if screen == Screen::Search {
        test.app.screen = Screen::Files;
        test.app.apply_action(Action::OpenSearch);
    }
    test.sent();
    test
}

#[test]
fn ctrl_p_opens_the_palette_from_every_screen() {
    for screen in SCREENS {
        let mut test = screen_of(screen);

        ctrl_p(&mut test);

        assert!(test.app.palette.is_some(), "{screen:?}");
        assert_eq!(test.app.screen, screen);
    }
}

#[test]
fn ctrl_p_opens_it_while_a_filter_is_being_typed_and_finishes_the_filter() {
    let mut test = on(Screen::Files);
    test.list_local(vec![entry(Path::new("/d"), "alpha", false)]);
    press(&mut test, KeyCode::Char('/'));
    test.type_text("al");
    assert!(test.app.local.editing_filter());

    ctrl_p(&mut test);

    assert!(test.app.palette.is_some());
    assert!(!test.app.local.editing_filter());
    assert_eq!(test.app.local.filter(), Some("al"));
}

#[test]
fn it_does_not_open_over_a_dialog_the_help_overlay_or_the_details_popup() {
    let mut dialog = on(Screen::Files);
    dialog.app.apply_action(Action::Mkdir);
    ctrl_p(&mut dialog);
    assert!(dialog.app.palette.is_none());
    assert!(dialog.app.dialog.is_some());

    let mut help = on(Screen::Files);
    help.app.apply_action(Action::Help);
    ctrl_p(&mut help);
    assert!(help.app.palette.is_none());
    assert!(!help.app.help_visible);

    let mut details = on(Screen::Connections);
    details.app.connections.replace(vec![connection("prod", "sftp", ConnectionSource::Profile)]);
    let _ = render_app(&details.app, 80, 24);
    details.app.apply_action(Action::ToggleSelect);
    assert!(details.app.details_popup);
    ctrl_p(&mut details);
    assert!(details.app.palette.is_none());
    assert!(!details.app.details_popup);
}

#[test]
fn opening_directly_is_a_no_op_while_something_else_owns_the_keyboard() {
    let mut dialog = on(Screen::Files);
    dialog.app.apply_action(Action::Mkdir);
    dialog.app.open_palette();
    assert!(dialog.app.palette.is_none());

    let mut help = on(Screen::Files);
    help.app.help_visible = true;
    help.app.open_palette();
    assert!(help.app.palette.is_none());

    let mut details = on(Screen::Files);
    details.app.details_popup = true;
    details.app.open_palette();
    assert!(details.app.palette.is_none());
}

#[test]
fn ctrl_p_again_closes_it() {
    let mut test = on(Screen::Files);
    ctrl_p(&mut test);

    ctrl_p(&mut test);

    assert!(test.app.palette.is_none());
}

#[test]
fn the_palette_is_drawn_over_the_screen_at_120x30() {
    let mut test = on(Screen::Files);
    test.list_local(vec![entry(Path::new("/d"), "visible.txt", false)]);
    test.app.theme =
        crate::theme::Theme::new(crate::theme::ColorDepth::Indexed, &crate::theme::ThemeSettings::default());
    open_with(&mut test, "new");

    let terminal = render_app(&test.app, 120, 30);

    assert!(find_text(&terminal, "\u{203a} new").is_some());
    assert!(find_text(&terminal, "esc close").is_some());
    assert!(find_text(&terminal, "New connection").is_some());
    let (x, y) = find_text(&terminal, "visible.txt").expect("the screen behind is still drawn");
    assert_eq!(cell_fg(&terminal, x, y), test.app.theme.line_strong);
}

#[test]
fn typing_goes_into_the_query_and_triggers_nothing() {
    for screen in SCREENS {
        let mut test = screen_of(screen);
        ctrl_p(&mut test);

        test.type_text(TYPED);

        assert_eq!(palette_query(&test), TYPED, "{screen:?}");
        assert!(test.app.palette.is_some());
        test.assert_inert(screen);
        assert!(test.app.dialog.is_none());
    }
}

#[test]
fn the_editing_keys_edit_the_query() {
    let mut test = on(Screen::Files);
    open_with(&mut test, "abc");

    press(&mut test, KeyCode::Left);
    press(&mut test, KeyCode::Backspace);
    assert_eq!(palette_query(&test), "ac");
    press(&mut test, KeyCode::Home);
    press(&mut test, KeyCode::Delete);
    assert_eq!(palette_query(&test), "c");
    press(&mut test, KeyCode::End);
    press(&mut test, KeyCode::Char('z'));
    press(&mut test, KeyCode::Right);
    assert_eq!(palette_query(&test), "cz");
}

#[test]
fn up_down_page_up_and_page_down_move_the_selection() {
    let mut test = on(Screen::Files);
    ctrl_p(&mut test);
    let selected = |test: &TestApp| test.app.palette.as_ref().unwrap().selected();

    press(&mut test, KeyCode::Down);
    assert_eq!(selected(&test), 1);
    press(&mut test, KeyCode::PageDown);
    assert_eq!(selected(&test), 6);
    press(&mut test, KeyCode::Up);
    assert_eq!(selected(&test), 5);
    press(&mut test, KeyCode::PageUp);
    assert_eq!(selected(&test), 0);
    press(&mut test, KeyCode::Up);
    assert_eq!(selected(&test), 0);
}

#[test]
fn unrelated_keys_are_ignored() {
    let mut test = on(Screen::Files);
    ctrl_p(&mut test);
    test.sent();

    for code in [KeyCode::Tab, KeyCode::F(5), KeyCode::F(10), KeyCode::Insert, KeyCode::BackTab] {
        press(&mut test, code);
    }
    test.app.apply_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    test.app.apply_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));

    assert!(test.app.palette.is_some());
    assert_eq!(palette_query(&test), "");
    test.assert_inert(Screen::Files);
}

type Trace = (Screen, usize, Option<String>, usize, Option<String>, Vec<Command>);

fn trace(test: &mut TestApp) -> Trace {
    (
        test.app.screen,
        test.app.local.cursor,
        test.app.local.filter().map(str::to_string),
        test.app.connections.cursor,
        test.notification(),
        test.sent(),
    )
}

#[test]
fn esc_closes_and_leaves_no_trace() {
    for screen in SCREENS {
        let mut test = screen_of(screen);
        test.list_local(vec![entry(Path::new("/d"), "a", false), entry(Path::new("/d"), "b", false)]);
        test.app.local.cursor = 1;
        test.app.connections.replace(vec![connection("prod", "sftp", ConnectionSource::Profile)]);
        let before = trace(&mut test);

        open_with(&mut test, "con");
        press(&mut test, KeyCode::Down);
        press(&mut test, KeyCode::Esc);

        assert!(test.app.palette.is_none(), "{screen:?}");
        assert_eq!(trace(&mut test), before, "{screen:?}");
    }
}

#[test]
fn a_second_esc_goes_back_as_usual() {
    let mut test = on(Screen::Connections);
    ctrl_p(&mut test);

    press(&mut test, KeyCode::Esc);
    assert_eq!(test.app.screen, Screen::Connections);
    press(&mut test, KeyCode::Esc);

    assert_eq!(test.app.screen, Screen::Files);
}

#[test]
fn enter_on_new_connection_opens_the_add_form_after_closing_the_palette() {
    let mut test = on(Screen::Files);
    open_with(&mut test, "new conn");

    press(&mut test, KeyCode::Enter);

    assert!(test.app.palette.is_none());
    assert!(matches!(test.app.dialog, Some(Dialog::Form(_))));
}

#[test]
fn new_connection_from_the_connections_screen_opens_the_form_too() {
    let mut test = on(Screen::Connections);
    open_with(&mut test, "new conn");

    press(&mut test, KeyCode::Enter);

    assert_eq!(test.app.screen, Screen::Connections);
    assert!(matches!(test.app.dialog, Some(Dialog::Form(_))));
}

#[test]
fn new_connection_from_the_files_screen_goes_to_the_connections_screen_first() {
    let mut test = on(Screen::Files);
    open_with(&mut test, "new conn");

    press(&mut test, KeyCode::Enter);

    assert_eq!(test.app.screen, Screen::Connections);
}

#[test]
fn every_listed_action_runs_on_its_screen_without_panicking() {
    for screen in SCREENS {
        let actions: Vec<Action> = screen_of(screen)
            .app
            .palette_entries()
            .into_iter()
            .filter_map(|entry| match entry.target {
                Target::Action(action) => Some(action),
                _ => None,
            })
            .collect();
        for action in actions {
            let mut test = screen_of(screen);
            test.list_local(vec![entry(Path::new("/d"), "a.txt", false)]);
            test.app.apply_action(action);
        }
    }
}

#[test]
fn enter_on_an_action_runs_it_through_apply_action() {
    let mut files = on(Screen::Files);
    open_with(&mut files, "new folder");
    press(&mut files, KeyCode::Enter);
    assert!(matches!(files.app.dialog, Some(Dialog::TextInput(_))));

    let mut by_palette = on(Screen::Transfers);
    open_with(&mut by_palette, "clear fin");
    by_palette.sent();
    press(&mut by_palette, KeyCode::Enter);
    let mut by_action = on(Screen::Transfers);
    by_action.sent();
    by_action.app.apply_action(Action::Refresh);
    assert_eq!(by_palette.sent(), by_action.sent());
    assert!(by_palette.app.palette.is_none());
}

#[test]
fn enter_on_connect_sends_connect_for_that_profile_and_shows_the_files_screen() {
    let mut test = on(Screen::Connections);
    test.app.connections.replace(vec![connection("prod", "sftp", ConnectionSource::Profile)]);
    open_with(&mut test, "@prod");
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert_eq!(test.sent(), vec![Command::Connect { profile: "prod".to_string() }]);
    assert_eq!(test.app.screen, Screen::Files);
    assert!(test.app.palette.is_none());
}

#[test]
fn connect_to_an_already_connected_profile_activates_its_session_and_sends_nothing() {
    let mut test = on(Screen::Connections);
    test.app.connections.replace(vec![
        connection("a", "sftp", ConnectionSource::Profile),
        connection("b", "sftp", ConnectionSource::Profile),
    ]);
    test.connect(1, "a");
    test.connect(2, "b");
    assert_eq!(test.app.sessions.active_id(), Some(2));
    open_with(&mut test, "@connect a");
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert_eq!(test.app.sessions.active_id(), Some(1));
    assert!(test.sent().is_empty());
    assert_eq!(test.app.screen, Screen::Files);
}

#[test]
fn enter_on_disconnect_sends_disconnect_for_that_session() {
    let mut test = on(Screen::Files);
    test.connect(1, "a");
    open_with(&mut test, "@disconnect a");
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert_eq!(test.sent(), vec![Command::Disconnect { session: 1 }]);
}

#[test]
fn enter_on_a_file_focuses_its_panel_and_moves_the_cursor() {
    let mut local = on(Screen::Transfers);
    local.list_local(vec![
        entry(Path::new("/d"), "a", false),
        entry(Path::new("/d"), "b", false),
        entry(Path::new("/d"), "c", false),
    ]);
    local.app.active_panel = ActivePanel::Remote;
    open_with(&mut local, "#c");
    press(&mut local, KeyCode::Enter);
    assert_eq!(local.app.screen, Screen::Files);
    assert_eq!(local.app.active_panel, ActivePanel::Local);
    assert_eq!(local.app.local.current_entry_name(), Some("c"));

    let mut remote = on(Screen::Files);
    remote.connect(1, "prod");
    remote.list_remote(1, "/srv", vec![entry(Path::new("/srv"), "x", false), entry(Path::new("/srv"), "y", false)]);
    remote.app.active_panel = ActivePanel::Local;
    open_with(&mut remote, "#y");
    press(&mut remote, KeyCode::Enter);
    assert_eq!(remote.app.active_panel, ActivePanel::Remote);
    assert_eq!(remote.app.sessions.active().unwrap().panel.current_entry_name(), Some("y"));
}

#[test]
fn enter_with_no_hits_does_nothing_and_keeps_the_palette_open() {
    let mut test = on(Screen::Files);
    open_with(&mut test, "zzzzzz");
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert!(test.app.palette.is_some());
    test.assert_inert(Screen::Files);
    assert!(test.app.dialog.is_none());
}

#[test]
fn disconnecting_between_open_and_enter_runs_nothing() {
    let mut test = on(Screen::Files);
    test.connect(1, "a");
    open_with(&mut test, "@disconnect a");
    test.app.apply_core_event(Event::Disconnected { session: 1, name: "a".to_string() });
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert!(test.sent().is_empty());
    assert!(test.app.palette.is_none());
}

#[test]
fn a_file_that_disappeared_is_ignored() {
    let mut test = on(Screen::Transfers);
    test.list_local(vec![entry(Path::new("/d"), "gone.txt", false)]);
    test.app.active_panel = ActivePanel::Remote;
    open_with(&mut test, "#gone");
    test.list_local(vec![entry(Path::new("/d"), "other.txt", false)]);
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert_eq!(test.app.screen, Screen::Transfers);
    assert_eq!(test.app.active_panel, ActivePanel::Remote);
    assert!(test.sent().is_empty());
}

#[test]
fn a_remote_file_of_a_session_that_disappeared_is_ignored() {
    let mut test = on(Screen::Files);
    test.connect(1, "prod");
    test.list_remote(1, "/srv", vec![entry(Path::new("/srv"), "x", false)]);
    open_with(&mut test, "#x");
    test.app.apply_core_event(Event::Disconnected { session: 1, name: "prod".to_string() });
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert!(test.sent().is_empty());
    assert!(test.app.palette.is_none());
}

#[test]
fn a_connection_deleted_while_open_sends_nothing_and_warns() {
    let mut test = on(Screen::Connections);
    test.app.connections.replace(vec![connection("prod", "sftp", ConnectionSource::Profile)]);
    open_with(&mut test, "@prod");
    test.app.apply_core_event(Event::Profiles(Vec::new()));
    test.sent();

    press(&mut test, KeyCode::Enter);

    assert!(test.sent().is_empty());
    assert_eq!(test.app.notifications.current().map(|note| note.severity), Some(Severity::Warning));
    assert_eq!(test.app.screen, Screen::Connections);
}

#[test]
fn a_connection_named_with_an_escape_sequence_is_drawn_sanitised() {
    let mut test = on(Screen::Files);
    test.app.connections.replace(vec![connection("a\u{1b}[2J\u{202e}b", "sftp", ConnectionSource::Profile)]);
    open_with(&mut test, "@");

    let terminal = render_app(&test.app, 120, 30);

    assert!(!has_unprintable_cell(&terminal));
    assert!(find_text(&terminal, "Connect a?[2J?b").is_some());
}

#[test]
fn the_help_overlay_lists_open_palette_with_ctrl_p() {
    let mut test = on(Screen::Files);
    test.app.help_visible = true;

    let terminal = render_app(&test.app, 100, 70);

    let (_, y) = find_text(&terminal, "open_palette").expect("listed");
    assert!(screen_rows(&terminal)[usize::from(y)].contains("Ctrl+P"));
}

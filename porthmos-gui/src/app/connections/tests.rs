use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};
use porthmos_core::{Command, ConnectionState, Event, Location, Severity, VfsPath};

use crate::connections::{ManagerButtons, details_lines};
use crate::shortcuts::KeyPress;
use crate::testing::Harness;

use super::super::*;
use crate::connections::Screen;

pub(super) fn profile(name: &str, source: ConnectionSource) -> ConnectionEntry {
    ConnectionEntry {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: "h".to_string(),
        port: 22,
        username: "u".to_string(),
        password: None,
        options: Default::default(),
        group: None,
        tags: Vec::new(),
        saved_password: false,
        in_keyring: Vec::new(),
        source,
    }
}

fn grouped(name: &str, group: &str) -> ConnectionEntry {
    ConnectionEntry { group: Some(group.to_string()), ..profile(name, ConnectionSource::Profile) }
}

fn feed_profiles(harness: &mut Harness, names: &[&str]) {
    let entries = names.iter().map(|name| profile(name, ConnectionSource::Profile)).collect();
    harness.feed(Event::Profiles(entries));
}

pub(super) fn named(harness: &mut Harness, named: Named) {
    let key = Key::Named(named);
    let press = KeyPress { key: key.clone(), modified_key: key, modifiers: Modifiers::empty(), text: None };
    let window = harness.key_target();
    harness.press(Message::WindowKey(window, press));
}

pub(super) fn named_in_main(harness: &mut Harness, named: Named) {
    let key = Key::Named(named);
    let press = KeyPress { key: key.clone(), modified_key: key, modifiers: Modifiers::empty(), text: None };
    let window = harness.main_id();
    harness.press(Message::WindowKey(window, press));
}

pub(super) fn typed(harness: &mut Harness, text: &str) {
    let key = Key::Character(text.into());
    let press =
        KeyPress { key: key.clone(), modified_key: key, modifiers: Modifiers::empty(), text: Some(text.to_string()) };
    let window = harness.key_target();
    harness.press(Message::WindowKey(window, press));
}

pub(super) fn open_manager(harness: &mut Harness) {
    harness.press(Message::ManagerToggled);
    harness.sent();
}

fn connected(harness: &mut Harness, session: u64, name: &str) {
    harness.feed(Event::Connected { session, name: name.to_string(), shell_available: false });
}

#[test]
fn the_plus_button_opens_the_manager_and_asks_core_for_the_profiles() {
    let mut harness = Harness::new();
    harness.sent();

    harness.press(Message::ManagerToggled);

    assert!(harness.app.manager_open());
    assert_eq!(harness.sent(), vec![Command::ListProfiles]);
    harness.press(Message::ManagerClosed);
    assert!(!harness.app.manager_open());
    assert!(harness.sent().is_empty());
}

#[test]
fn escape_closes_the_manager() {
    let mut harness = Harness::new();
    open_manager(&mut harness);

    named(&mut harness, Named::Escape);

    assert!(!harness.app.manager_open());
}

#[test]
fn close_button_closes_the_manager() {
    let mut harness = Harness::new();
    open_manager(&mut harness);

    harness.press(Message::ManagerClosed);

    assert!(!harness.app.manager_open());
}

#[test]
fn file_shortcuts_do_nothing_while_the_manager_is_open() {
    let mut harness = Harness::new();
    let folder = VfsPath::new("/home/tester");
    let entries = vec![porthmos_core::Entry { name: "a".into(), path: folder.join("a"), ..Default::default() }];
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries });
    harness.press(Message::PanelClicked(PanelSide::Local, 0));
    open_manager(&mut harness);

    named(&mut harness, Named::Delete);
    named(&mut harness, Named::F5);
    named(&mut harness, Named::F7);
    typed(&mut harness, "n");

    assert!(harness.app.modal().is_none());
    assert!(harness.sent().is_empty());
}

#[test]
fn typing_in_the_filter_narrows_the_list() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);

    harness.press(Message::ManagerFilterTyped("alp".to_string()));

    assert_eq!(harness.app.manager().match_count(), 1);
    assert_eq!(harness.app.manager().filter(), "alp");
}

#[test]
fn connect_sends_connect_for_the_selected_profile_and_keeps_the_manager_open_until_it_is_connected() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(1));

    harness.press(Message::ManagerConnect);

    assert_eq!(harness.sent(), vec![Command::Connect { profile: "beta".to_string() }]);
    assert!(harness.app.manager_open());

    connected(&mut harness, 7, "beta");

    assert!(!harness.app.manager_open());
}

#[test]
fn a_connection_the_manager_did_not_ask_for_leaves_the_window_open() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(1));
    harness.press(Message::ManagerConnect);

    connected(&mut harness, 3, "alpha");

    assert!(harness.app.manager_open());
}

#[test]
fn a_failed_connection_leaves_the_window_open_and_a_later_connection_does_not_close_it() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));
    harness.press(Message::ManagerConnect);

    harness.feed(Event::ConnectFailed { name: "alpha".to_string(), message: "refused".into() });
    assert!(harness.app.manager_open());

    connected(&mut harness, 4, "alpha");
    assert!(harness.app.manager_open());
}

#[test]
fn connect_is_disabled_for_a_connection_that_is_being_connected() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));
    harness.press(Message::ManagerConnect);
    harness.feed(Event::Connecting { name: "alpha".to_string() });
    harness.sent();

    assert!(!harness.app.manager_buttons().connect);
    harness.press(Message::ManagerConnect);
    assert!(harness.sent().is_empty());

    harness.press(Message::ManagerRowPressed(1));
    assert!(harness.app.manager_buttons().connect);
}

#[test]
fn the_connect_button_offers_to_switch_to_a_connection_that_is_already_open() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    connected(&mut harness, 1, "alpha");
    open_manager(&mut harness);

    harness.press(Message::ManagerRowPressed(0));
    assert_eq!(harness.app.connect_label(), "Switch to");

    harness.press(Message::ManagerRowPressed(1));
    assert_eq!(harness.app.connect_label(), "Connect");
}

#[test]
fn a_lost_connection_is_offered_as_connect_again() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    connected(&mut harness, 1, "alpha");
    harness.feed(Event::SessionHealth { session: 1, healthy: false });
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));

    assert_eq!(harness.app.connect_label(), "Connect");
}

#[test]
fn enter_connects_the_selected_profile() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);
    named(&mut harness, Named::ArrowDown);
    named(&mut harness, Named::ArrowDown);

    named(&mut harness, Named::Enter);

    assert_eq!(harness.sent(), vec![Command::Connect { profile: "beta".to_string() }]);
}

#[test]
fn arrow_keys_move_the_selection() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);

    named(&mut harness, Named::ArrowDown);
    assert_eq!(harness.app.manager().selected_entry().unwrap().name, "alpha");
    named(&mut harness, Named::ArrowDown);
    assert_eq!(harness.app.manager().selected_entry().unwrap().name, "beta");
    named(&mut harness, Named::ArrowUp);
    assert_eq!(harness.app.manager().selected_entry().unwrap().name, "alpha");
}

#[test]
fn double_click_on_a_row_connects() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);

    harness.press(Message::ManagerRowActivated(0));

    assert_eq!(harness.sent(), vec![Command::Connect { profile: "alpha".to_string() }]);
}

#[test]
fn connecting_to_a_healthy_open_session_only_activates_its_tab() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    connected(&mut harness, 1, "alpha");
    connected(&mut harness, 2, "beta");
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));

    harness.press(Message::ManagerConnect);

    assert!(harness.sent().is_empty());
    assert_eq!(harness.app.sessions().active().unwrap().id, 1);
    assert!(!harness.app.manager_open());
}

#[test]
fn connecting_to_an_unhealthy_session_disconnects_it_first() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    connected(&mut harness, 1, "alpha");
    harness.feed(Event::SessionHealth { session: 1, healthy: false });
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));

    harness.press(Message::ManagerConnect);

    assert_eq!(
        harness.sent(),
        vec![Command::Disconnect { session: 1 }, Command::Connect { profile: "alpha".to_string() }]
    );
}

#[test]
fn connecting_to_a_name_that_is_gone_warns() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    harness.sent();

    let started = harness.app.connect_profile("zzz");

    assert!(!started);
    let notice = harness.app.notices().last().unwrap();
    assert_eq!(notice.severity, Severity::Warning);
    assert_eq!(notice.message, "That connection no longer exists");
    assert!(harness.sent().is_empty());
}

#[test]
fn the_connected_event_leaves_the_manager_open() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    open_manager(&mut harness);

    connected(&mut harness, 1, "alpha");

    assert!(harness.app.manager_open());
}

#[test]
fn pressing_a_group_toggles_it_and_selects_no_connection() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![grouped("alpha", "Work")]));
    open_manager(&mut harness);
    assert_eq!(harness.app.manager().rows().len(), 1);

    harness.press(Message::ManagerGroupToggled("Work".to_string()));

    assert_eq!(harness.app.manager().rows().len(), 2);
    assert!(harness.app.manager().selected_entry().is_none());
    assert_eq!(harness.app.manager_buttons(), ManagerButtons { connect: false, edit: false, delete: false });
}

#[test]
fn buttons_are_disabled_without_a_selection() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    open_manager(&mut harness);

    assert_eq!(harness.app.manager_buttons(), ManagerButtons { connect: false, edit: false, delete: false });
}

#[test]
fn a_saved_profile_enables_every_button() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));

    assert_eq!(harness.app.manager_buttons(), ManagerButtons { connect: true, edit: true, delete: true });
}

#[test]
fn an_ssh_config_host_can_connect_and_edit_but_not_delete() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![profile("host", ConnectionSource::SshConfig)]));
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));

    assert_eq!(harness.app.manager_buttons(), ManagerButtons { connect: true, edit: true, delete: false });
}

#[test]
fn an_orphan_label_can_only_be_edited() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![profile("gone", ConnectionSource::MissingSshHost)]));
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));

    assert_eq!(harness.app.manager_buttons(), ManagerButtons { connect: false, edit: true, delete: false });
}

#[test]
fn an_orphan_label_never_connects() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![profile("gone", ConnectionSource::MissingSshHost)]));
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(0));

    harness.press(Message::ManagerConnect);

    assert!(harness.sent().is_empty());
}

#[test]
fn panel_rows_stay_pressable_while_the_manager_is_open() {
    let mut harness = Harness::new();
    let folder = VfsPath::new("/home/tester");
    let entries = vec![porthmos_core::Entry { name: "a".into(), path: folder.join("a"), ..Default::default() }];
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries });
    open_manager(&mut harness);

    harness.press(Message::RowPressed(PanelSide::Local, 0));

    assert!(harness.app.drag.is_tracking());
}

#[test]
fn details_skip_empty_values_and_show_the_start_folder() {
    let mut entry = profile("alpha", ConnectionSource::Profile);
    entry.options.insert("remote_path".to_string(), "/srv".to_string());
    entry.saved_password = true;

    let lines = details_lines(&entry, "SFTP", ConnectionState::Disconnected);

    assert_eq!(
        lines,
        vec![
            ("Protocol".to_string(), "SFTP".to_string()),
            ("Host".to_string(), "h".to_string()),
            ("Port".to_string(), "22".to_string()),
            ("User".to_string(), "u".to_string()),
            ("Saved password".to_string(), "yes".to_string()),
            ("Start folder".to_string(), "/srv".to_string()),
            ("Status".to_string(), "Not connected".to_string()),
        ]
    );
}

#[test]
fn details_show_group_tags_and_connected_status() {
    let mut entry = grouped("alpha", "Work/Db");
    entry.tags = vec!["prod".to_string(), "eu".to_string()];

    let lines = details_lines(&entry, "SFTP", ConnectionState::Connected);

    assert!(lines.contains(&("Group".to_string(), "Work/Db".to_string())));
    assert!(lines.contains(&("Tags".to_string(), "prod, eu".to_string())));
    assert_eq!(lines.last().unwrap(), &("Status".to_string(), "Connected".to_string()));
}

pub(super) fn harness() -> Harness {
    Harness::with_builtin_protocols()
}

pub(super) fn saved(name: &str) -> ConnectionEntry {
    ConnectionEntry {
        host: "example.com".to_string(),
        username: "me".to_string(),
        ..profile(name, ConnectionSource::Profile)
    }
}

pub(super) fn with_profiles(entries: Vec<ConnectionEntry>) -> Harness {
    let mut harness = harness();
    harness.feed(Event::Profiles(entries));
    open_manager(&mut harness);
    harness
}

pub(super) fn select(harness: &mut Harness, row: usize) {
    harness.press(Message::ManagerRowPressed(row));
}

pub(super) fn type_field(harness: &mut Harness, key: &str, value: &str) {
    harness.press(Message::FormTextTyped(key.to_string(), value.to_string()));
}

pub(super) fn form(harness: &Harness) -> &crate::connections::form::FormState {
    match harness.app.screen() {
        Some(Screen::Form(state)) => state,
        other => panic!("{other:?}"),
    }
}

pub(super) fn fill_new(harness: &mut Harness) {
    type_field(harness, "name", "prod");
    type_field(harness, "host", "example.com");
    type_field(harness, "username", "me");
}

#[test]
fn details_report_a_lost_connection() {
    let entry = profile("alpha", ConnectionSource::Profile);

    let lines = details_lines(&entry, "SFTP", ConnectionState::Lost);

    assert_eq!(lines.last().unwrap(), &("Status".to_string(), "Connection lost".to_string()));
}

#[test]
fn the_link_state_follows_the_state_core_publishes() {
    let mut harness = harness();
    feed_profiles(&mut harness, &["alpha"]);
    assert_eq!(harness.app.link_state("alpha"), ConnectionState::Disconnected);

    for state in [
        ConnectionState::Connecting,
        ConnectionState::Connected,
        ConnectionState::Lost,
        ConnectionState::Connected,
        ConnectionState::Disconnected,
    ] {
        harness.feed(Event::ConnectionState { name: "alpha".to_string(), state });
        assert_eq!(harness.app.link_state("alpha"), state);
    }
}

#[test]
fn connecting_a_lost_connection_reconnects_it() {
    let mut harness = harness();
    feed_profiles(&mut harness, &["alpha"]);
    connected(&mut harness, 1, "alpha");
    harness.feed(Event::SessionHealth { session: 1, healthy: false });
    harness.press(Message::ManagerToggled);
    harness.sent();
    select(&mut harness, 0);

    harness.press(Message::ManagerConnect);

    assert_eq!(
        harness.sent(),
        vec![Command::Disconnect { session: 1 }, Command::Connect { profile: "alpha".to_string() }]
    );
}

#[test]
fn the_list_names_a_protocol_the_way_the_form_does() {
    let harness = harness();

    assert_eq!(harness.app.protocol_name("webdav"), "WebDAV");
    assert_eq!(harness.app.protocol_name("sftp"), "SFTP");
    assert_eq!(harness.app.protocol_name("unknown"), "unknown");
}

fn window_of(harness: &Harness) -> Option<iced::window::Id> {
    harness.app.window_id_of(crate::windows::WindowKind::Connections)
}

#[test]
fn opening_the_manager_creates_one_connections_window() {
    let mut harness = Harness::new();

    open_manager(&mut harness);

    assert!(window_of(&harness).is_some());
    assert_eq!(harness.connections_id(), window_of(&harness));
}

#[test]
fn opening_the_manager_again_raises_the_window_and_keeps_it_open() {
    let mut harness = Harness::new();
    open_manager(&mut harness);
    let first = window_of(&harness);

    harness.press(Message::ManagerToggled);

    assert!(harness.app.manager_open());
    assert_eq!(window_of(&harness), first);
    assert!(harness.sent().is_empty());
}

#[test]
fn closing_the_manager_closes_its_window() {
    let mut harness = Harness::new();
    open_manager(&mut harness);

    harness.press(Message::ManagerClosed);

    assert_eq!(window_of(&harness), None);
}

#[test]
fn the_close_request_clears_the_manager_form_and_filter_state() {
    let mut harness = Harness::with_builtin_protocols();
    open_manager(&mut harness);
    harness.press(Message::ManagerFilterTyped("prod".to_string()));
    harness.press(Message::ManagerAddPressed);
    let id = window_of(&harness).unwrap();

    harness.press(Message::WindowCloseRequested(id));

    assert!(!harness.app.manager_open());
    assert_eq!(window_of(&harness), None);
    assert!(!harness.app.is_closing());
    open_manager(&mut harness);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
}

#[test]
fn escape_inside_the_form_closes_the_form_first_then_the_window() {
    let mut harness = Harness::with_builtin_protocols();
    open_manager(&mut harness);
    harness.press(Message::ManagerAddPressed);
    assert!(matches!(harness.app.screen(), Some(Screen::Form(_))));

    named(&mut harness, Named::Escape);
    assert!(matches!(harness.app.screen(), Some(Screen::List)));
    assert!(window_of(&harness).is_some());

    named(&mut harness, Named::Escape);
    assert_eq!(window_of(&harness), None);
}

#[test]
fn the_connections_window_closes_once_the_connection_is_made() {
    let mut harness = Harness::new();
    open_manager(&mut harness);
    feed_profiles(&mut harness, &["alpha"]);
    harness.press(Message::ManagerRowPressed(0));
    harness.sent();

    harness.press(Message::ManagerConnect);

    assert!(harness.sent().contains(&Command::Connect { profile: "alpha".to_string() }));
    assert!(harness.app.manager_open());
    connected(&mut harness, 1, "alpha");
    assert!(!harness.app.manager_open());
}

#[test]
fn keys_in_the_connections_window_do_not_reach_the_panels() {
    let mut harness = Harness::new();
    open_manager(&mut harness);
    let cursor = harness.app.local().panel.cursor();
    harness.sent();

    typed(&mut harness, "j");
    named(&mut harness, Named::Delete);
    named(&mut harness, Named::F5);

    assert_eq!(harness.app.local().panel.cursor(), cursor);
    assert!(harness.app.modal().is_none());
}

#[test]
fn keys_in_the_main_window_do_not_reach_the_manager() {
    let mut harness = Harness::new();
    open_manager(&mut harness);
    let main = harness.main_id();
    let key = Key::Named(Named::Escape);

    harness.press(Message::WindowKey(
        main,
        KeyPress { key: key.clone(), modified_key: key, modifiers: Modifiers::empty(), text: None },
    ));

    assert!(harness.app.manager_open());
}

#[test]
fn the_main_window_stays_usable_while_the_manager_is_open() {
    let mut harness = Harness::new();
    open_manager(&mut harness);
    harness.press(Message::PathEditStarted(PanelSide::Local));

    assert!(harness.app.path_edit().is_some());
    assert!(harness.app.manager_open());
}

#[test]
fn the_connections_window_builds_its_view_for_every_screen() {
    let mut harness = Harness::with_builtin_protocols();
    open_manager(&mut harness);
    let id = window_of(&harness).unwrap();
    drop(harness.app.view_of(id));
    harness.press(Message::ManagerAddPressed);
    drop(harness.app.view_of(id));
}

#[test]
fn resizing_the_connections_window_changes_only_its_own_size() {
    let mut harness = Harness::new();
    open_manager(&mut harness);
    let id = window_of(&harness).unwrap();
    let main_before = harness.app.window_size();

    harness.press(Message::WindowSized(id, iced::Size::new(700.0, 480.0)));

    assert_eq!(harness.app.connections_window_size(), iced::Size::new(700.0, 480.0));
    assert_eq!(harness.app.window_size(), main_before);
}

#[test]
fn opening_the_manager_a_second_time_raises_the_window_with_one_task() {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);

    let units = harness.app.update(Message::ManagerToggled).units();

    assert_eq!(units, 1);
}

#[test]
fn the_filter_is_cleared_when_the_window_closes() {
    let mut harness = Harness::new();
    harness.press(Message::ManagerToggled);
    harness.press(Message::ManagerFilterTyped("prod".to_string()));
    assert_eq!(harness.app.manager.filter(), "prod");

    harness.press(Message::ManagerClosed);
    harness.press(Message::ManagerToggled);

    assert_eq!(harness.app.manager.filter(), "");
}

fn remembered_groups(commands: Vec<Command>) -> Vec<Vec<String>> {
    commands
        .into_iter()
        .filter_map(|command| match command {
            Command::RememberOpenGroups { groups } => Some(groups),
            _ => None,
        })
        .collect()
}

#[test]
fn toggling_a_group_tells_core_which_groups_are_open() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![grouped("alpha", "Work")]));
    open_manager(&mut harness);
    harness.sent();

    harness.press(Message::ManagerGroupToggled("Work".to_string()));
    harness.press(Message::ManagerGroupToggled("Work".to_string()));

    assert_eq!(remembered_groups(harness.sent()), vec![vec!["Work".to_string()], Vec::<String>::new()]);
}

#[test]
fn a_saved_connection_that_reveals_its_group_opens_it_for_core_too() {
    let mut harness = Harness::new();
    harness.feed(Event::Profiles(vec![grouped("alpha", "Work")]));
    harness.sent();
    harness.app.reveal = Some("alpha".to_string());

    harness.feed(Event::Profiles(vec![grouped("alpha", "Work")]));

    assert_eq!(remembered_groups(harness.sent()), vec![vec!["Work".to_string()]]);
}

#[test]
fn the_window_app_starts_with_the_groups_core_remembered() {
    let mut harness = Harness::with_remembered_open_groups(vec!["Work".to_string()]);

    harness.feed(Event::Profiles(vec![grouped("alpha", "Work")]));

    assert_eq!(harness.app.manager().rows().len(), 2);
}

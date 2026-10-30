use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};
use porthmos_core::{Command, Event, Location, Severity, VfsPath};

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
    harness.press(Message::KeyPressed(press));
}

pub(super) fn typed(harness: &mut Harness, text: &str) {
    let key = Key::Character(text.into());
    let press =
        KeyPress { key: key.clone(), modified_key: key, modifiers: Modifiers::empty(), text: Some(text.to_string()) };
    harness.press(Message::KeyPressed(press));
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
    harness.press(Message::ManagerToggled);
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
fn connect_sends_connect_for_the_selected_profile_and_closes_the_manager() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha", "beta"]);
    open_manager(&mut harness);
    harness.press(Message::ManagerRowPressed(1));

    harness.press(Message::ManagerConnect);

    assert_eq!(harness.sent(), vec![Command::Connect { profile: "beta".to_string() }]);
    assert!(!harness.app.manager_open());
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
fn the_connected_event_closes_the_manager() {
    let mut harness = Harness::new();
    feed_profiles(&mut harness, &["alpha"]);
    open_manager(&mut harness);

    connected(&mut harness, 1, "alpha");

    assert!(!harness.app.manager_open());
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
fn panel_rows_cannot_be_pressed_while_the_manager_is_open() {
    let mut harness = Harness::new();
    let folder = VfsPath::new("/home/tester");
    let entries = vec![porthmos_core::Entry { name: "a".into(), path: folder.join("a"), ..Default::default() }];
    harness.feed(Event::Listed { location: Location::Local, path: folder, entries });
    open_manager(&mut harness);

    harness.press(Message::RowPressed(PanelSide::Local, 0));

    assert!(!harness.app.drag.is_tracking());
}

#[test]
fn details_skip_empty_values_and_show_the_start_folder() {
    let mut entry = profile("alpha", ConnectionSource::Profile);
    entry.options.insert("remote_path".to_string(), "/srv".to_string());
    entry.saved_password = true;

    let lines = details_lines(&entry, "SFTP", false);

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

    let lines = details_lines(&entry, "SFTP", true);

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

use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};
use porthmos_core::{Command, Event, Severity};

use super::super::*;
use super::QUICK_CONNECTION_LIMIT;
use crate::testing::Harness;

fn entry(name: &str, source: ConnectionSource) -> ConnectionEntry {
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

fn feed(harness: &mut Harness, entries: Vec<ConnectionEntry>) {
    harness.feed(Event::Profiles(entries));
    harness.sent();
}

fn profiles(names: &[&str]) -> Vec<ConnectionEntry> {
    names.iter().map(|name| entry(name, ConnectionSource::Profile)).collect()
}

#[test]
fn there_is_nothing_to_offer_before_core_lists_the_connections() {
    let harness = Harness::new();

    assert!(harness.app.quick_connections().is_empty());
}

#[test]
fn saved_connections_are_offered_in_the_order_core_lists_them() {
    let mut harness = Harness::new();
    feed(&mut harness, profiles(&["nas", "prod", "staging"]));

    assert_eq!(harness.app.quick_connections(), vec!["nas", "prod", "staging"]);
}

#[test]
fn ssh_config_hosts_are_offered_too() {
    let mut harness = Harness::new();
    feed(&mut harness, vec![entry("box", ConnectionSource::SshConfig)]);

    assert_eq!(harness.app.quick_connections(), vec!["box"]);
}

#[test]
fn rows_that_cannot_be_connected_are_never_offered() {
    let mut harness = Harness::new();
    feed(
        &mut harness,
        vec![
            entry("real", ConnectionSource::Profile),
            entry("gone", ConnectionSource::MissingSshHost),
            entry("hidden", ConnectionSource::ShadowedSshHost),
        ],
    );

    assert_eq!(harness.app.quick_connections(), vec!["real"]);
}

#[test]
fn only_the_first_few_are_offered() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..QUICK_CONNECTION_LIMIT + 3).map(|index| format!("c{index}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    feed(&mut harness, profiles(&refs));

    assert_eq!(harness.app.quick_connections(), names[..QUICK_CONNECTION_LIMIT].to_vec());
}

#[test]
fn the_limit_is_five() {
    assert_eq!(QUICK_CONNECTION_LIMIT, 5);
}

#[test]
fn the_remainder_is_counted_so_the_panel_can_point_to_the_manager() {
    let mut harness = Harness::new();
    let names: Vec<String> = (0..QUICK_CONNECTION_LIMIT + 3).map(|index| format!("c{index}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    feed(&mut harness, profiles(&refs));

    assert_eq!(harness.app.quick_connections_hidden(), 3);
}

#[test]
fn nothing_is_hidden_when_everything_fits() {
    let mut harness = Harness::new();
    feed(&mut harness, profiles(&["a", "b"]));

    assert_eq!(harness.app.quick_connections_hidden(), 0);
}

#[test]
fn pressing_a_quick_connection_asks_core_to_connect_it() {
    let mut harness = Harness::new();
    feed(&mut harness, profiles(&["nas", "prod"]));

    harness.press(Message::QuickConnectPressed("prod".to_string()));

    assert_eq!(harness.sent(), vec![Command::Connect { profile: "prod".to_string() }]);
}

#[test]
fn a_connection_deleted_meanwhile_gives_a_warning_and_no_command() {
    let mut harness = Harness::new();
    feed(&mut harness, profiles(&["nas"]));

    harness.press(Message::QuickConnectPressed("old".to_string()));

    assert!(harness.sent().is_empty());
    assert!(harness.app.notices().iter().any(|notice| notice.severity == Severity::Warning));
}

#[test]
fn the_add_button_opens_the_connection_manager() {
    let mut harness = Harness::new();

    harness.press(Message::ManagerToggled);

    assert!(harness.app.manager_open());
}

fn used(harness: &mut Harness, list: &[(&str, u32)]) {
    use chrono::TimeZone;
    let times = list
        .iter()
        .map(|(name, day)| (name.to_string(), chrono::Utc.with_ymd_and_hms(2026, 10, *day, 12, 0, 0).unwrap()))
        .collect();
    harness.feed(Event::LastUsed(times));
}

#[test]
fn recently_used_connections_come_first_newest_first_then_the_rest_in_listed_order() {
    let mut harness = Harness::new();
    feed(&mut harness, profiles(&["a", "b", "c", "d"]));
    used(&mut harness, &[("c", 3), ("b", 9)]);

    assert_eq!(harness.app.quick_connections(), vec!["b", "c", "a", "d"]);
}

#[test]
fn a_last_used_entry_for_a_deleted_connection_is_ignored() {
    let mut harness = Harness::new();
    feed(&mut harness, profiles(&["a", "b"]));
    used(&mut harness, &[("gone", 9)]);

    assert_eq!(harness.app.quick_connections(), vec!["a", "b"]);
}

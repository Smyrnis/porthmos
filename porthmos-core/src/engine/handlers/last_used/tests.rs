use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use chrono::{DateTime, TimeZone, Utc};
use porthmos_vfs::testing::{FakeFs, FakeProtocol};

use crate::{
    Severity,
    engine::{
        Command, Event,
        testing::{TestEngine, test_engine},
    },
    profiles::{ConnectionProfile, Labels, ProfileDraft, store},
};

fn morning() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, 9, 0, 0).unwrap()
}

fn evening() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, 18, 30, 0).unwrap()
}

fn engine_with_profile(name: &str) -> TestEngine {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.config_dir).unwrap();
    std::fs::write(
        t.engine.paths.connections_file(),
        format!("[connections.{name}]\nprotocol = \"fake\"\nhost = \"h\"\nusername = \"u\"\n"),
    )
    .unwrap();
    t.engine.protocols = vec![Arc::new(FakeProtocol::new(FakeFs::new()))];
    t.engine.clock = morning;
    t
}

fn engine_discovering(names: &[&str]) -> TestEngine {
    let mut t = test_engine();
    let discovered = names
        .iter()
        .map(|name| porthmos_vfs::Target {
            name: name.to_string(),
            host: format!("{name}.example"),
            port: 22,
            username: "u".into(),
            password: None,
            options: Default::default(),
        })
        .collect();
    let protocol = FakeProtocol::new(FakeFs::new()).with_id("sftp").with_discovered(discovered);
    t.engine.protocols = vec![Arc::new(protocol)];
    t.engine.clock = morning;
    t
}

fn published(events: &[Event]) -> Vec<BTreeMap<String, DateTime<Utc>>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::LastUsed(times) => Some(times.clone()),
            _ => None,
        })
        .collect()
}

async fn connect(t: &mut TestEngine, name: &str) {
    t.engine.handle_command(Command::Connect { profile: name.into() });
    t.run_internal().await;
}

async fn file_on_disk(t: &mut TestEngine) -> String {
    t.engine.writer.flush().await;
    std::fs::read_to_string(t.engine.paths.last_used_file()).unwrap_or_default()
}

fn times(pairs: &[(&str, DateTime<Utc>)]) -> BTreeMap<String, DateTime<Utc>> {
    pairs.iter().map(|(name, at)| (name.to_string(), *at)).collect()
}

fn remember(t: &mut TestEngine, name: &str, at: DateTime<Utc>) {
    t.engine.last_used.store.record(name, at);
}

#[tokio::test]
async fn a_successful_connection_records_the_engine_clock_and_publishes_it() {
    let mut t = engine_with_profile("srv");

    connect(&mut t, "srv").await;

    assert_eq!(published(&t.drain()), [times(&[("srv", morning())])]);
    assert_eq!(t.engine.last_used.store.get("srv"), Some(morning()));
}

#[tokio::test]
async fn the_recorded_time_reaches_the_file() {
    let mut t = engine_with_profile("srv");

    connect(&mut t, "srv").await;

    assert_eq!(file_on_disk(&mut t).await.trim(), "[last_used]\nsrv = \"2026-10-22T09:00:00Z\"");
}

#[cfg(unix)]
#[tokio::test]
async fn the_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let mut t = engine_with_profile("srv");

    connect(&mut t, "srv").await;
    file_on_disk(&mut t).await;

    let mode = std::fs::metadata(t.engine.paths.last_used_file()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[tokio::test]
async fn a_failed_connection_records_nothing() {
    let mut t = engine_with_profile("srv");
    t.engine.protocols = Vec::new();

    t.engine.handle_command(Command::Connect { profile: "srv".into() });

    assert!(published(&t.drain()).is_empty());
    assert_eq!(t.engine.last_used.store.get("srv"), None);
    assert_eq!(file_on_disk(&mut t).await, "");
}

#[tokio::test]
async fn connecting_to_an_unknown_profile_records_nothing() {
    let mut t = engine_with_profile("srv");

    t.engine.handle_command(Command::Connect { profile: "ghost".into() });

    assert!(t.engine.last_used.store.times().is_empty());
}

#[tokio::test]
async fn reusing_an_open_session_keeps_the_time_of_the_connection_that_opened_it() {
    let mut t = engine_with_profile("srv");
    connect(&mut t, "srv").await;
    t.drain();
    t.engine.clock = evening;

    t.engine.handle_command(Command::Connect { profile: "srv".into() });

    assert!(published(&t.drain()).is_empty());
    assert_eq!(t.engine.last_used.store.get("srv"), Some(morning()));
}

#[tokio::test]
async fn reconnecting_after_a_disconnect_records_the_newer_time() {
    let mut t = engine_with_profile("srv");
    connect(&mut t, "srv").await;
    let session = *t.engine.sessions.keys().next().unwrap();
    t.engine.handle_command(Command::Disconnect { session });
    t.drain();
    t.engine.clock = evening;

    connect(&mut t, "srv").await;

    assert_eq!(published(&t.drain()), [times(&[("srv", evening())])]);
}

#[tokio::test]
async fn a_ssh_config_host_records_its_time_too() {
    let mut t = engine_discovering(&["web1"]);

    connect(&mut t, "web1").await;

    assert_eq!(t.engine.last_used.store.get("web1"), Some(morning()));
}

#[test]
fn publishing_sends_every_recorded_time() {
    let mut t = test_engine();
    remember(&mut t, "a", morning());
    remember(&mut t, "b", evening());

    t.engine.publish_last_used();

    assert_eq!(published(&t.drain()), [times(&[("a", morning()), ("b", evening())])]);
}

#[test]
fn publishing_with_nothing_recorded_sends_an_empty_map() {
    let mut t = test_engine();

    t.engine.publish_last_used();

    assert_eq!(published(&t.drain()), [BTreeMap::new()]);
}

fn profile(name: &str) -> ConnectionProfile {
    ConnectionProfile {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host: "server.example.com".to_string(),
        port: Some(22),
        username: "deploy".to_string(),
        options: BTreeMap::new(),
        group: None,
        tags: Vec::new(),
        in_keyring: Vec::new(),
    }
}

fn engine_saving_sftp() -> TestEngine {
    let mut t = test_engine();
    t.engine.protocols = vec![Arc::new(FakeProtocol::new(FakeFs::new()).with_id("sftp"))];
    store::save(&t.engine.paths, &profile("prod")).unwrap();
    t
}

fn draft(name: &str) -> ProfileDraft {
    ProfileDraft {
        name: name.to_string(),
        host: "server.example.com".to_string(),
        port: "22".to_string(),
        username: "deploy".to_string(),
        protocol: "sftp".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn renaming_a_connection_carries_its_time_to_the_new_name() {
    let mut t = engine_saving_sftp();
    remember(&mut t, "prod", morning());

    t.engine
        .handle_command(Command::SaveProfile { original: Some("prod".into()), draft: Box::new(draft("production")) });

    assert_eq!(published(&t.drain()), [times(&[("production", morning())])]);
    assert_eq!(t.engine.last_used.store.get("prod"), None);
    assert_eq!(file_on_disk(&mut t).await.trim(), "[last_used]\nproduction = \"2026-10-22T09:00:00Z\"");
}

#[tokio::test]
async fn editing_without_renaming_leaves_the_time_alone() {
    let mut t = engine_saving_sftp();
    remember(&mut t, "prod", morning());

    t.engine.handle_command(Command::SaveProfile { original: Some("prod".into()), draft: Box::new(draft("prod")) });

    assert!(published(&t.drain()).is_empty());
    assert_eq!(t.engine.last_used.store.get("prod"), Some(morning()));
}

#[tokio::test]
async fn a_rejected_rename_keeps_the_time_where_it_was() {
    let mut t = engine_saving_sftp();
    store::save(&t.engine.paths, &profile("taken")).unwrap();
    remember(&mut t, "prod", morning());

    t.engine.handle_command(Command::SaveProfile { original: Some("prod".into()), draft: Box::new(draft("taken")) });

    assert_eq!(t.engine.last_used.store.get("prod"), Some(morning()));
    assert_eq!(t.engine.last_used.store.get("taken"), None);
}

#[tokio::test]
async fn saving_a_new_connection_gives_it_no_time() {
    let mut t = engine_saving_sftp();

    t.engine.handle_command(Command::SaveProfile { original: None, draft: Box::new(draft("fresh")) });

    assert!(t.engine.last_used.store.times().is_empty());
}

#[tokio::test]
async fn deleting_a_connection_forgets_its_time() {
    let mut t = engine_saving_sftp();
    remember(&mut t, "prod", morning());
    remember(&mut t, "other", evening());

    t.engine.handle_command(Command::DeleteProfile { name: "prod".into() });

    assert_eq!(published(&t.drain()), [times(&[("other", evening())])]);
    assert_eq!(file_on_disk(&mut t).await.trim(), "[last_used]\nother = \"2026-10-22T18:30:00Z\"");
}

#[tokio::test]
async fn deleting_a_connection_that_was_never_used_publishes_nothing() {
    let mut t = engine_saving_sftp();

    t.engine.handle_command(Command::DeleteProfile { name: "prod".into() });

    assert!(published(&t.drain()).is_empty());
}

fn group_only() -> Labels {
    Labels { group: Some("A".to_string()), tags: Vec::new(), in_keyring: Vec::new() }
}

#[tokio::test]
async fn moving_ssh_labels_carries_the_time_to_the_new_host() {
    let mut t = engine_discovering(&["new"]);
    store::save_ssh_labels(&t.engine.paths, "old", &group_only()).unwrap();
    remember(&mut t, "old", morning());

    t.engine.handle_command(Command::MoveSshLabels { from: "old".into(), to: "new".into() });

    assert_eq!(published(&t.drain()), [times(&[("new", morning())])]);
}

#[tokio::test]
async fn moving_ssh_labels_to_an_unknown_host_keeps_the_time() {
    let mut t = engine_discovering(&["new"]);
    store::save_ssh_labels(&t.engine.paths, "old", &group_only()).unwrap();
    remember(&mut t, "old", morning());

    t.engine.handle_command(Command::MoveSshLabels { from: "old".into(), to: "ghost".into() });

    assert_eq!(t.engine.last_used.store.get("old"), Some(morning()));
}

#[tokio::test]
async fn forgetting_ssh_labels_forgets_the_time() {
    let mut t = engine_discovering(&[]);
    store::save_ssh_labels(&t.engine.paths, "old", &group_only()).unwrap();
    remember(&mut t, "old", morning());

    t.engine.handle_command(Command::ForgetSshLabels { name: "old".into() });

    assert_eq!(published(&t.drain()), [BTreeMap::new()]);
    assert_eq!(t.engine.last_used.store.get("old"), None);
}

#[tokio::test]
async fn a_connection_that_fails_after_the_handshake_started_records_nothing() {
    let mut t = engine_with_profile("srv");
    t.engine.protocols = vec![Arc::new(FakeProtocol::new(FakeFs::new()).failing_connect("Connection refused"))];

    t.engine.handle_command(Command::Connect { profile: "srv".into() });
    t.run_internal().await;

    let events = t.drain();
    assert!(events.iter().any(|event| matches!(event, Event::ConnectFailed { .. })), "{events:?}");
    assert!(published(&events).is_empty());
    assert_eq!(t.engine.last_used.store.get("srv"), None);
    assert_eq!(file_on_disk(&mut t).await, "");
}

#[tokio::test]
async fn a_connection_finishing_after_its_profile_was_deleted_records_nothing() {
    let mut t = engine_with_profile("srv");

    t.engine.handle_command(Command::Connect { profile: "srv".into() });
    t.engine.handle_command(Command::DeleteProfile { name: "srv".into() });
    t.run_internal().await;

    assert!(t.engine.last_used.store.times().is_empty());
    assert!(published(&t.drain()).is_empty());
}

#[tokio::test]
async fn a_connection_finishing_after_its_profile_was_renamed_records_nothing_under_the_old_name() {
    let mut t = engine_with_profile("srv");
    let draft = ProfileDraft {
        name: "srv2".to_string(),
        host: "h".to_string(),
        port: "22".to_string(),
        username: "u".to_string(),
        protocol: "fake".to_string(),
        ..Default::default()
    };

    t.engine.handle_command(Command::Connect { profile: "srv".into() });
    t.engine.handle_command(Command::SaveProfile { original: Some("srv".into()), draft: Box::new(draft) });
    t.run_internal().await;

    assert!(t.engine.last_used.store.times().is_empty());
}

fn engine_with_profile_shadowing_a_host(name: &str, other_host: &str) -> TestEngine {
    let mut t = engine_with_profile(name);
    let hosts = [name, other_host]
        .iter()
        .map(|host| porthmos_vfs::Target {
            name: host.to_string(),
            host: format!("{host}.example"),
            port: 22,
            username: "u".into(),
            password: None,
            options: Default::default(),
        })
        .collect();
    t.engine.protocols = vec![Arc::new(FakeProtocol::new(FakeFs::new()).with_discovered(hosts))];
    store::save_ssh_labels(&t.engine.paths, name, &group_only()).unwrap();
    t
}

#[tokio::test]
async fn forgetting_the_labels_of_a_hidden_host_keeps_the_time_of_the_profile_that_hides_it() {
    let mut t = engine_with_profile_shadowing_a_host("web", "other");
    remember(&mut t, "web", morning());

    t.engine.handle_command(Command::ForgetSshLabels { name: "web".into() });

    assert_eq!(t.engine.last_used.store.get("web"), Some(morning()));
    assert!(published(&t.drain()).is_empty());
}

#[tokio::test]
async fn moving_the_labels_of_a_hidden_host_leaves_the_time_of_the_profile_that_hides_it() {
    let mut t = engine_with_profile_shadowing_a_host("web", "other");
    remember(&mut t, "web", morning());

    t.engine.handle_command(Command::MoveSshLabels { from: "web".into(), to: "other".into() });

    assert_eq!(t.engine.last_used.store.get("web"), Some(morning()));
    assert_eq!(t.engine.last_used.store.get("other"), None);
}

#[tokio::test]
async fn moving_ssh_labels_never_replaces_a_newer_time_of_the_new_host() {
    let mut t = engine_discovering(&["new"]);
    store::save_ssh_labels(&t.engine.paths, "old", &group_only()).unwrap();
    remember(&mut t, "old", morning());
    remember(&mut t, "new", evening());

    t.engine.handle_command(Command::MoveSshLabels { from: "old".into(), to: "new".into() });

    assert_eq!(published(&t.drain()), [times(&[("new", evening())])]);
}

#[tokio::test]
async fn renaming_an_unused_connection_onto_a_name_with_a_stale_time_clears_it() {
    let mut t = engine_saving_sftp();
    remember(&mut t, "web", morning());

    t.engine.handle_command(Command::SaveProfile { original: Some("prod".into()), draft: Box::new(draft("web")) });

    assert_eq!(t.engine.last_used.store.get("web"), None);
    assert_eq!(published(&t.drain()), [BTreeMap::new()]);
}

#[tokio::test]
async fn a_failed_write_is_a_warning_once_and_debug_after() {
    let mut t = engine_with_profile("srv");
    t.engine.persist_failed(t.engine.paths.last_used_file(), "disk full".to_string(), 0);

    let first = t.notices();
    t.engine.persist_failed(t.engine.paths.last_used_file(), "disk full".to_string(), 0);
    let second = t.notices();

    assert!(
        matches!(first.as_slice(), [(Severity::Warning, message)] if message.contains("last-used") && message.contains("disk full")),
        "{first:?}"
    );
    assert!(second.is_empty(), "{second:?}");
}

#[tokio::test]
async fn a_history_write_failure_is_not_blamed_on_the_last_used_file() {
    let mut t = engine_with_profile("srv");

    t.engine.persist_failed(PathBuf::from("history.toml"), "disk full".to_string(), 0);

    let notices = t.notices();
    assert!(notices.iter().all(|(_, message)| !message.contains("last-used")), "{notices:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_that_could_not_be_set_aside_is_never_overwritten() {
    let mut t = engine_with_profile("srv");
    t.engine.last_used.store = protected_store(&t);
    t.drain();

    connect(&mut t, "srv").await;
    let session = *t.engine.sessions.keys().next().unwrap();
    t.engine.handle_command(Command::Disconnect { session });
    connect(&mut t, "srv").await;

    assert_eq!(file_on_disk(&mut t).await, "not toml [");
    let warnings: Vec<_> = t
        .notices()
        .into_iter()
        .filter(|(severity, message)| *severity == Severity::Warning && message.contains("last-used"))
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[cfg(unix)]
fn protected_store(t: &TestEngine) -> crate::last_used::LastUsed {
    use std::os::unix::fs::PermissionsExt;
    let paths = &t.engine.paths;
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), "not toml [").unwrap();
    std::fs::set_permissions(&paths.state_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let (store, _warning) = crate::last_used::LastUsed::load(paths);
    std::fs::set_permissions(&paths.state_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!store.is_writable());
    store
}

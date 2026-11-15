use std::sync::Arc;

use porthmos_vfs::testing::{FakeFs, FakeProtocol};

use crate::{
    config::ReopenMode,
    engine::{
        Command, Event,
        testing::{TestEngine, test_engine},
    },
    state::{self, UiState},
};

fn profile_lines(names: &[(&str, &str)]) -> String {
    names
        .iter()
        .map(|(name, protocol)| {
            format!("[connections.{name}]\nprotocol = \"{protocol}\"\nhost = \"h\"\nusername = \"u\"\n\n")
        })
        .collect()
}

fn engine_with_profiles(mode: ReopenMode, names: &[(&str, &str)]) -> TestEngine {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.config_dir).unwrap();
    std::fs::write(t.engine.paths.connections_file(), profile_lines(names)).unwrap();
    t.engine.protocols = vec![
        Arc::new(FakeProtocol::new(FakeFs::new())),
        Arc::new(FakeProtocol::new(FakeFs::new()).with_id("failing").failing_connect("Connection refused")),
    ];
    t.engine.reopen.mode = mode;
    t
}

fn remember_last_run(t: &mut TestEngine, names: &[&str]) {
    let state = UiState { reopen: names.iter().map(|name| name.to_string()).collect(), ..UiState::default() };
    state::save(&t.engine.paths, &state).unwrap();
    t.engine.load_ui_state();
    t.drain();
}

fn saved_list(t: &TestEngine) -> Vec<String> {
    state::load(&t.engine.paths).state.reopen
}

fn offers(events: &[Event]) -> Vec<Vec<String>> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ReopenOffer { names } => Some(names.clone()),
            _ => None,
        })
        .collect()
}

fn connecting(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Connecting { name } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn connected(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Connected { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

async fn finish_one(t: &mut TestEngine) {
    let pending: Vec<String> = t.engine.connecting.iter().cloned().collect();
    loop {
        t.run_internal().await;
        if pending.iter().any(|name| !t.engine.connecting.contains(name)) {
            break;
        }
    }
}

#[tokio::test]
async fn connections_are_recorded_in_connection_order_when_the_mode_asks() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake"), ("b", "fake")]);

    t.engine.handle_command(Command::Connect { profile: "b".into() });
    finish_one(&mut t).await;
    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;

    assert_eq!(saved_list(&t), vec!["b", "a"]);
}

#[tokio::test]
async fn connections_are_recorded_when_the_mode_reopens_automatically() {
    let mut t = engine_with_profiles(ReopenMode::Auto, &[("a", "fake")]);

    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;

    assert_eq!(saved_list(&t), vec!["a"]);
}

#[tokio::test]
async fn nothing_is_recorded_while_the_mode_is_off() {
    let mut t = engine_with_profiles(ReopenMode::Off, &[("a", "fake")]);

    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;

    assert!(saved_list(&t).is_empty());
    assert!(!t.engine.paths.state_file().exists());
}

#[tokio::test]
async fn disconnecting_removes_only_that_connection() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake"), ("b", "fake")]);
    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;
    t.engine.handle_command(Command::Connect { profile: "b".into() });
    finish_one(&mut t).await;
    let first = *t.engine.sessions.iter().find(|(_, live)| live.name == "a").unwrap().0;

    t.engine.handle_command(Command::Disconnect { session: first });

    assert_eq!(saved_list(&t), vec!["b"]);
}

#[tokio::test]
async fn disconnecting_the_last_connection_leaves_nothing_to_reopen() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;
    let session = *t.engine.sessions.keys().next().unwrap();

    t.engine.handle_command(Command::Disconnect { session });

    assert!(saved_list(&t).is_empty());
}

#[tokio::test]
async fn a_failed_connection_is_not_recorded() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("bad", "failing")]);

    t.engine.handle_command(Command::Connect { profile: "bad".into() });
    finish_one(&mut t).await;

    assert!(saved_list(&t).is_empty());
}

#[tokio::test]
async fn the_same_connection_is_listed_once() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    t.add_session("a");
    t.add_session("a");

    t.engine.remember_open_connections();

    assert_eq!(saved_list(&t), vec!["a"]);
}

#[tokio::test]
async fn nothing_is_offered_when_the_mode_is_off() {
    let mut t = engine_with_profiles(ReopenMode::Off, &[("a", "fake")]);
    remember_last_run(&mut t, &["a"]);

    t.engine.offer_reopen();

    assert!(offers(&t.drain()).is_empty());
}

#[tokio::test]
async fn nothing_is_offered_when_the_last_run_left_no_connections() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    remember_last_run(&mut t, &[]);

    t.engine.offer_reopen();

    assert!(offers(&t.drain()).is_empty());
}

#[tokio::test]
async fn the_last_runs_connections_are_offered_without_connecting() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake"), ("b", "fake")]);
    remember_last_run(&mut t, &["b", "a"]);

    t.engine.offer_reopen();

    let events = t.drain();
    assert_eq!(offers(&events), vec![vec!["b".to_string(), "a".to_string()]]);
    assert!(connecting(&events).is_empty());
    assert!(t.engine.sessions.is_empty());
}

#[tokio::test]
async fn connections_that_no_longer_exist_are_left_out_of_the_offer() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    remember_last_run(&mut t, &["gone", "a"]);

    t.engine.offer_reopen();

    assert_eq!(offers(&t.drain()), vec![vec!["a".to_string()]]);
}

#[tokio::test]
async fn nothing_is_offered_when_none_of_the_connections_exist_any_more() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    remember_last_run(&mut t, &["gone"]);

    t.engine.offer_reopen();

    assert!(offers(&t.drain()).is_empty());
}

#[tokio::test]
async fn accepting_the_offer_connects_them_one_at_a_time_in_order() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake"), ("b", "fake")]);
    remember_last_run(&mut t, &["a", "b"]);
    t.engine.offer_reopen();
    t.drain();

    t.engine.handle_command(Command::ReopenConnections);
    let first = t.drain();
    assert_eq!(connecting(&first), vec!["a"]);

    finish_one(&mut t).await;
    let second = t.drain();
    assert_eq!(connected(&second), vec!["a"]);
    assert_eq!(connecting(&second), vec!["b"]);

    finish_one(&mut t).await;
    assert_eq!(connected(&t.drain()), vec!["b"]);
    assert_eq!(saved_list(&t), vec!["a", "b"]);
}

#[tokio::test]
async fn a_connection_that_fails_does_not_stop_the_ones_after_it() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("bad", "failing"), ("b", "fake")]);
    remember_last_run(&mut t, &["bad", "b"]);
    t.engine.offer_reopen();
    t.drain();

    t.engine.handle_command(Command::ReopenConnections);
    finish_one(&mut t).await;
    let events = t.drain();

    assert!(events.iter().any(|event| matches!(event, Event::ConnectFailed { name, .. } if name == "bad")));
    assert_eq!(connecting(&events), vec!["bad", "b"]);
    finish_one(&mut t).await;
    assert_eq!(connected(&t.drain()), vec!["b"]);
}

#[tokio::test]
async fn a_connection_that_is_already_open_is_not_opened_twice() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake"), ("b", "fake")]);
    remember_last_run(&mut t, &["a", "b"]);
    t.engine.offer_reopen();
    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;
    t.drain();

    t.engine.handle_command(Command::ReopenConnections);
    finish_one(&mut t).await;

    assert_eq!(t.engine.sessions.values().filter(|live| live.name == "a").count(), 1);
    assert_eq!(t.engine.sessions.values().filter(|live| live.name == "b").count(), 1);
}

#[tokio::test]
async fn the_offer_keeps_the_last_runs_list_even_after_other_connections_were_made() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake"), ("b", "fake"), ("c", "fake")]);
    remember_last_run(&mut t, &["a", "b"]);
    t.engine.offer_reopen();
    t.engine.handle_command(Command::Connect { profile: "c".into() });
    finish_one(&mut t).await;
    t.drain();

    t.engine.handle_command(Command::ReopenConnections);
    finish_one(&mut t).await;
    finish_one(&mut t).await;

    let mut names: Vec<_> = t.engine.sessions.values().map(|live| live.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn accepting_the_offer_twice_connects_only_once() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    remember_last_run(&mut t, &["a"]);
    t.engine.offer_reopen();
    t.engine.handle_command(Command::ReopenConnections);
    finish_one(&mut t).await;
    t.drain();

    t.engine.handle_command(Command::ReopenConnections);

    assert!(connecting(&t.drain()).is_empty());
    assert_eq!(t.engine.sessions.len(), 1);
}

#[tokio::test]
async fn asking_to_reopen_with_no_offer_does_nothing() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);

    t.engine.handle_command(Command::ReopenConnections);

    assert!(t.drain().is_empty());
}

#[tokio::test]
async fn the_automatic_mode_starts_connecting_at_once_and_offers_nothing() {
    let mut t = engine_with_profiles(ReopenMode::Auto, &[("a", "fake"), ("b", "fake")]);
    remember_last_run(&mut t, &["a", "b"]);

    t.engine.offer_reopen();
    let first = t.drain();
    assert_eq!(connecting(&first), vec!["a"]);
    assert!(offers(&first).is_empty());

    finish_one(&mut t).await;
    finish_one(&mut t).await;
    assert_eq!(t.engine.sessions.len(), 2);
}

#[tokio::test]
async fn switching_the_mode_off_forgets_the_remembered_connections() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;
    assert_eq!(saved_list(&t), vec!["a"]);
    t.drain();

    t.engine.handle_command(Command::SetReopenMode { mode: ReopenMode::Off });

    assert!(saved_list(&t).is_empty());
    t.engine.handle_command(Command::ReopenConnections);
    assert!(connecting(&t.drain()).is_empty());
}

#[tokio::test]
async fn switching_the_mode_on_remembers_what_is_open_right_now() {
    let mut t = engine_with_profiles(ReopenMode::Off, &[("a", "fake")]);
    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;
    assert!(saved_list(&t).is_empty());

    t.engine.handle_command(Command::SetReopenMode { mode: ReopenMode::Ask });

    assert_eq!(saved_list(&t), vec!["a"]);
}

#[tokio::test]
async fn switching_the_mode_off_drops_a_pending_offer() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    remember_last_run(&mut t, &["a"]);
    t.engine.offer_reopen();
    t.drain();

    t.engine.handle_command(Command::SetReopenMode { mode: ReopenMode::Off });
    t.engine.handle_command(Command::ReopenConnections);

    assert!(connecting(&t.drain()).is_empty());
    assert!(t.engine.sessions.is_empty());
}

#[tokio::test]
async fn a_protected_state_file_is_left_alone() {
    let mut t = engine_with_profiles(ReopenMode::Ask, &[("a", "fake")]);
    t.engine.state_protected = true;

    t.engine.handle_command(Command::Connect { profile: "a".into() });
    finish_one(&mut t).await;

    assert!(!t.engine.paths.state_file().exists());
}

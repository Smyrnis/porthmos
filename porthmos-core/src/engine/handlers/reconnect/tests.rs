use std::{sync::Arc, time::Duration};

use porthmos_vfs::{
    ErrorKind, VfsPath,
    testing::{FakeFs, FakeProtocol},
};

use super::*;
use crate::{
    Severity,
    engine::{
        Command, Event, Internal, SessionId,
        testing::{TestEngine, test_engine},
    },
};

fn profile_engine(protocol: impl FnOnce(FakeFs) -> FakeProtocol) -> (TestEngine, FakeFs) {
    let mut t = test_engine();
    std::fs::create_dir_all(&t.engine.paths.config_dir).unwrap();
    std::fs::write(
        t.engine.paths.connections_file(),
        "[connections.srv]\nprotocol = \"fake\"\nhost = \"h\"\nusername = \"u\"\n",
    )
    .unwrap();
    let remote = FakeFs::new();
    t.engine.protocols = vec![Arc::new(protocol(remote.clone()))];
    t.engine.reconnect.enabled = true;
    (t, remote)
}

fn lost_session(t: &mut TestEngine, folder: &str) -> SessionId {
    let session = t.add_session_with("srv", Arc::new(FakeFs::new()));
    t.engine.session_folders.insert(session, VfsPath::from(folder));
    t.engine.session_health_changed(session, false);
    t.drain();
    session
}

async fn next_due(t: &mut TestEngine) -> (String, u32) {
    loop {
        let done = tokio::time::timeout(Duration::from_secs(600), t.internal.recv()).await.unwrap().unwrap();
        if let Internal::ReconnectDue { name, attempt } = done {
            return (name, attempt);
        }
    }
}

async fn attempt(t: &mut TestEngine) -> Vec<Event> {
    let (name, attempt) = next_due(t).await;
    t.engine.handle_internal(Internal::ReconnectDue { name, attempt });
    t.run_internal().await;
    t.drain()
}

fn notice_texts(events: &[Event], wanted: Severity) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Notice { severity, message } if *severity == wanted => Some(message.to_string()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_lost_connection_is_retried_after_the_first_delay() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    let session = t.add_session_with("srv", Arc::new(FakeFs::new()));
    t.drain();

    t.engine.session_health_changed(session, false);
    tokio::time::sleep(RETRY_DELAYS[0] - Duration::from_millis(1)).await;
    assert!(t.internal.try_recv().is_err());
    tokio::time::sleep(Duration::from_millis(2)).await;

    let Internal::ReconnectDue { name, attempt } = t.internal.try_recv().unwrap() else {
        panic!("expected a reconnect timer");
    };
    assert_eq!((name.as_str(), attempt), ("srv", 0));
}

#[tokio::test(start_paused = true)]
async fn an_attempt_replaces_the_lost_session_and_returns_to_its_folder() {
    let (mut t, remote) = profile_engine(FakeProtocol::new);
    remote.file("/home/user/a.txt", b"", None).file("/srv/logs/b.log", b"", None);
    let lost = lost_session(&mut t, "/srv/logs");

    let (name, number) = next_due(&mut t).await;
    t.engine.handle_internal(Internal::ReconnectDue { name, attempt: number });
    t.run_internal().await;
    let mut events = t.drain();
    while !events.iter().any(|event| matches!(event, Event::Listed { .. })) {
        events.push(t.next_event().await);
    }

    assert!(events.iter().any(|event| matches!(event, Event::Disconnected { session, .. } if *session == lost)));
    assert!(events.iter().any(|event| matches!(event, Event::Connected { name, .. } if name == "srv")));
    let listed = events.iter().find_map(|event| match event {
        Event::Listed { path, .. } => Some(path.clone()),
        _ => None,
    });
    assert_eq!(listed, Some(VfsPath::from("/srv/logs")));
}

#[tokio::test(start_paused = true)]
async fn a_successful_reconnect_says_so() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    lost_session(&mut t, "/home/user");

    let events = attempt(&mut t).await;

    assert!(notice_texts(&events, Severity::Info).contains(&"srv: reconnected".to_string()), "{events:?}");
    assert!(t.engine.reconnect.is_idle());
}

#[tokio::test(start_paused = true)]
async fn each_failed_attempt_waits_longer_than_the_last() {
    let (mut t, _remote) = profile_engine(|fs| FakeProtocol::new(fs).failing_connect("refused"));
    lost_session(&mut t, "/home/user");

    for (index, delay) in RETRY_DELAYS.iter().enumerate() {
        let started = tokio::time::Instant::now();
        let (name, number) = next_due(&mut t).await;
        assert_eq!(number as usize, index);
        assert_eq!(started.elapsed(), *delay);
        t.engine.handle_internal(Internal::ReconnectDue { name, attempt: number });
        t.run_internal().await;
        if index + 1 < RETRY_DELAYS.len() {
            t.drain();
        }
    }
}

#[tokio::test(start_paused = true)]
async fn after_the_last_attempt_it_gives_up_and_says_how_to_continue() {
    let (mut t, _remote) = profile_engine(|fs| FakeProtocol::new(fs).failing_connect("refused"));
    lost_session(&mut t, "/home/user");

    let mut last = Vec::new();
    for _ in RETRY_DELAYS {
        last = attempt(&mut t).await;
    }

    assert!(
        notice_texts(&last, Severity::Warning)
            .iter()
            .any(|text| text == "srv: could not reconnect \u{2014} connect again by hand"),
        "{last:?}"
    );
    assert!(t.engine.reconnect.is_idle());
    tokio::time::sleep(Duration::from_secs(3600)).await;
    assert!(t.internal.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn a_connection_that_comes_back_by_itself_is_left_alone() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    let session = lost_session(&mut t, "/home/user");

    t.engine.session_health_changed(session, true);
    let (name, number) = next_due(&mut t).await;
    t.engine.handle_internal(Internal::ReconnectDue { name, attempt: number });

    let events = t.drain();
    assert!(!events.iter().any(|event| matches!(event, Event::Disconnected { .. } | Event::Connecting { .. })));
    assert!(t.engine.sessions.contains_key(&session));
}

#[tokio::test(start_paused = true)]
async fn disconnecting_by_hand_cancels_the_retries() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    let session = lost_session(&mut t, "/home/user");

    t.engine.handle_command(Command::Disconnect { session });
    t.drain();
    let (name, number) = next_due(&mut t).await;
    t.engine.handle_internal(Internal::ReconnectDue { name, attempt: number });

    assert!(!t.drain().iter().any(|event| matches!(event, Event::Connecting { .. })));
    assert!(t.engine.reconnect.is_idle());
}

#[tokio::test(start_paused = true)]
async fn a_second_loss_of_the_same_connection_does_not_start_a_second_chain() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    let session = lost_session(&mut t, "/home/user");

    t.engine.session_health_changed(session, false);
    tokio::time::sleep(RETRY_DELAYS[0] * 2).await;

    let mut due = 0;
    while let Ok(done) = t.internal.try_recv() {
        due += usize::from(matches!(done, Internal::ReconnectDue { .. }));
    }
    assert_eq!(due, 1);
}

#[tokio::test(start_paused = true)]
async fn rejected_credentials_stop_the_retries_so_the_account_is_not_locked() {
    let (mut t, _remote) = profile_engine(|fs| FakeProtocol::new(fs).failing_with(ErrorKind::AuthRejected, "denied"));
    lost_session(&mut t, "/home/user");

    attempt(&mut t).await;

    assert!(t.engine.reconnect.is_idle());
    tokio::time::sleep(Duration::from_secs(3600)).await;
    assert!(t.internal.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn declining_a_prompt_stops_the_retries() {
    let (mut t, _remote) = profile_engine(|fs| FakeProtocol::new(fs).failing_with(ErrorKind::Cancelled, "declined"));
    lost_session(&mut t, "/home/user");

    attempt(&mut t).await;

    assert!(t.engine.reconnect.is_idle());
    tokio::time::sleep(Duration::from_secs(3600)).await;
    assert!(t.internal.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn a_stale_timer_is_ignored() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    lost_session(&mut t, "/home/user");

    t.engine.handle_internal(Internal::ReconnectDue { name: "srv".into(), attempt: 3 });

    assert!(t.drain().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_profile_that_is_gone_ends_the_retries() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    lost_session(&mut t, "/home/user");
    std::fs::write(t.engine.paths.connections_file(), "").unwrap();

    let (name, number) = next_due(&mut t).await;
    t.engine.handle_internal(Internal::ReconnectDue { name, attempt: number });

    assert!(t.engine.reconnect.is_idle());
}

#[tokio::test(start_paused = true)]
async fn nothing_is_retried_while_the_setting_is_off() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    t.engine.reconnect.enabled = false;

    lost_session(&mut t, "/home/user");
    tokio::time::sleep(Duration::from_secs(3600)).await;

    assert!(t.internal.try_recv().is_err());
    assert!(t.engine.reconnect.is_idle());
}

#[tokio::test(start_paused = true)]
async fn the_setting_is_off_unless_the_user_turns_it_on() {
    let t = test_engine();

    assert!(!t.engine.reconnect.enabled);
}

#[tokio::test(start_paused = true)]
async fn turning_the_setting_on_applies_to_the_next_loss() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    t.engine.reconnect.enabled = false;

    t.engine.handle_command(Command::SetReconnect { enabled: true });
    lost_session(&mut t, "/home/user");

    let (name, number) = next_due(&mut t).await;
    assert_eq!((name.as_str(), number), ("srv", 0));
}

#[tokio::test(start_paused = true)]
async fn turning_the_setting_off_cancels_a_pending_retry() {
    let (mut t, _remote) = profile_engine(FakeProtocol::new);
    let session = lost_session(&mut t, "/home/user");

    t.engine.handle_command(Command::SetReconnect { enabled: false });
    let (name, number) = next_due(&mut t).await;
    t.engine.handle_internal(Internal::ReconnectDue { name, attempt: number });

    assert!(!t.drain().iter().any(|event| matches!(event, Event::Disconnected { .. } | Event::Connecting { .. })));
    assert!(t.engine.sessions.contains_key(&session));
    assert!(t.engine.reconnect.is_idle());
}

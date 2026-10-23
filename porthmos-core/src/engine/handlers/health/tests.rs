use std::{sync::Arc, time::Duration};

use porthmos_vfs::testing::{FakeFs, FakeProtocol};

use crate::{
    Severity,
    engine::{
        Event, Internal, SessionId,
        testing::{TestEngine, sample_entry, test_engine},
    },
    health::PROBE_INTERVAL,
};

fn watched(t: &mut TestEngine) -> (SessionId, FakeFs) {
    let fs = FakeFs::new().holding_connection();
    let session = t.add_session_with("srv", Arc::new(fs.clone()));
    t.engine.start_health_probe(session);
    (session, fs)
}

async fn next_health(t: &mut TestEngine) -> Internal {
    loop {
        let done = tokio::time::timeout(Duration::from_secs(600), t.internal.recv()).await.unwrap().unwrap();
        if matches!(done, Internal::SessionHealth { .. }) {
            return done;
        }
    }
}

fn health_events(events: &[Event]) -> Vec<(SessionId, bool)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::SessionHealth { session, healthy } => Some((*session, *healthy)),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_session_that_keeps_answering_produces_no_health_news() {
    let mut t = test_engine();
    let (_session, _fs) = watched(&mut t);

    tokio::time::sleep(PROBE_INTERVAL * 6).await;

    assert!(t.internal.try_recv().is_err());
    assert!(t.drain().is_empty());
}

#[tokio::test(start_paused = true)]
async fn losing_the_connection_marks_the_session_lost_and_warns() {
    let mut t = test_engine();
    let (session, fs) = watched(&mut t);
    t.drain();
    fs.lose_connection();

    let done = next_health(&mut t).await;
    t.engine.handle_internal(done);
    let events = t.drain();

    assert_eq!(health_events(&events), vec![(session, false)]);
    let warning = events
        .iter()
        .find_map(|event| match event {
            Event::Notice { severity: Severity::Warning, message } => Some(message.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(warning, "srv: connection lost");
}

#[tokio::test(start_paused = true)]
async fn a_connection_that_stays_lost_is_reported_once() {
    let mut t = test_engine();
    let (_session, fs) = watched(&mut t);
    fs.lose_connection();
    next_health(&mut t).await;

    tokio::time::sleep(PROBE_INTERVAL * 6).await;

    assert!(t.internal.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn the_connection_coming_back_marks_the_session_healthy_and_says_so() {
    let mut t = test_engine();
    let (session, fs) = watched(&mut t);
    fs.lose_connection();
    let lost = next_health(&mut t).await;
    t.engine.handle_internal(lost);
    t.drain();
    fs.restore_connection();

    let back = next_health(&mut t).await;
    t.engine.handle_internal(back);
    let events = t.drain();

    assert_eq!(health_events(&events), vec![(session, true)]);
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Notice { severity: Severity::Info, message } if message == "srv: connection restored"
    )));
}

#[tokio::test(start_paused = true)]
async fn a_session_that_holds_no_connection_stops_watching_at_once() {
    let mut t = test_engine();
    let session = t.add_session_with("web", Arc::new(FakeFs::new()));

    t.engine.start_health_probe(session);

    assert!(t.engine.tasks.wait_for_name("health-probe", Duration::from_secs(1)).await);
    assert!(t.internal.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn disconnecting_stops_the_probe() {
    let mut t = test_engine();
    let (session, _fs) = watched(&mut t);
    assert!(t.engine.tasks.live_names().contains(&"health-probe"));

    t.engine.disconnect(session);

    assert!(t.engine.tasks.wait_for_name("health-probe", Duration::from_secs(60)).await);
}

#[tokio::test(start_paused = true)]
async fn a_connection_lost_while_disconnecting_produces_no_news() {
    let mut t = test_engine();
    let (session, fs) = watched(&mut t);
    tokio::time::sleep(PROBE_INTERVAL / 2).await;
    fs.lose_connection();

    t.engine.disconnect(session);
    tokio::time::sleep(PROBE_INTERVAL * 3).await;

    assert!(t.internal.try_recv().is_err());
}

#[tokio::test(start_paused = true)]
async fn a_probe_result_for_a_session_that_is_gone_is_ignored() {
    let mut t = test_engine();
    t.drain();

    t.engine.handle_internal(Internal::SessionHealth { session: 99, healthy: false });

    assert!(t.drain().is_empty());
}

#[tokio::test(start_paused = true)]
async fn connecting_starts_the_probe_for_a_connection_based_protocol() {
    let mut t = test_engine();
    let fs = FakeFs::new().holding_connection();

    t.engine.finish_connect(sample_entry("srv"), Arc::new(FakeProtocol::new(FakeFs::new())), Arc::new(fs), None);

    assert!(t.engine.tasks.live_names().contains(&"health-probe"));
}

#[tokio::test(start_paused = true)]
async fn each_session_is_judged_on_its_own() {
    let mut t = test_engine();
    let (first, first_fs) = watched(&mut t);
    let second = t.add_session_with("other", Arc::new(FakeFs::new().holding_connection()));
    t.engine.start_health_probe(second);
    t.drain();
    first_fs.lose_connection();

    let done = next_health(&mut t).await;
    t.engine.handle_internal(done);
    tokio::time::sleep(PROBE_INTERVAL * 4).await;

    assert_eq!(health_events(&t.drain()), vec![(first, false)]);
    assert!(t.internal.try_recv().is_err());
}

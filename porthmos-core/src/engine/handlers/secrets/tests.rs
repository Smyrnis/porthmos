use std::{sync::Arc, time::Duration};

use crate::{
    engine::{Event, testing::test_engine_with_secrets},
    secrets::TestBackend,
};

fn waiting(events: &[Event]) -> Vec<bool> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::KeyringWaiting { waiting } => Some(*waiting),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn starting_probes_the_keyring_and_reports_it() {
    let backend = Arc::new(TestBackend::new());
    let mut t = test_engine_with_secrets(backend.clone());

    t.engine.start_keyring_probe();
    t.run_internal().await;

    let events = t.drain();
    assert!(matches!(events.as_slice(), [Event::KeyringStatus { available: true }]), "{events:?}");
    assert_eq!(backend.calls(), vec!["get porthmos:probe"]);
    assert!(t.engine.secrets.available());
}

#[tokio::test]
async fn a_failing_probe_reports_no_keyring() {
    let backend = Arc::new(TestBackend::new());
    backend.fail_reads();
    let mut t = test_engine_with_secrets(backend);

    t.engine.start_keyring_probe();
    t.run_internal().await;

    assert!(t.drain().iter().any(|event| matches!(event, Event::KeyringStatus { available: false })));
    assert!(!t.engine.secrets.available());
}

#[tokio::test]
async fn an_engine_without_a_keyring_reports_none_without_calling_anything() {
    let mut t = crate::engine::testing::test_engine();

    t.engine.start_keyring_probe();
    t.run_internal().await;

    assert!(t.drain().iter().any(|event| matches!(event, Event::KeyringStatus { available: false })));
}

#[tokio::test]
async fn a_slow_keyring_is_reported_as_waiting_and_then_done() {
    let backend = Arc::new(TestBackend::new());
    backend.block_for(Duration::from_millis(150));
    let mut t = test_engine_with_secrets(backend);

    t.engine.start_keyring_probe();
    t.run_internal().await;

    assert_eq!(waiting(&t.drain()), vec![true, false]);
}

#[tokio::test]
async fn a_fast_keyring_is_never_reported_as_waiting() {
    let backend = Arc::new(TestBackend::new());
    let mut t = test_engine_with_secrets(backend);

    t.engine.start_keyring_probe();
    t.run_internal().await;

    assert!(waiting(&t.drain()).is_empty());
}

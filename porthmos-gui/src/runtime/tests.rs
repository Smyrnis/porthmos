use std::time::{Duration, Instant};

use porthmos_core::{Command, CoreHandle, Environment, Event, Paths, UpdateMode};
use tokio::sync::mpsc::unbounded_channel;

use super::*;

#[tokio::test]
async fn started_carries_the_update_mode() {
    for (text, expected) in [("off", UpdateMode::Off), ("notify", UpdateMode::Notify), ("auto", UpdateMode::Auto)] {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(root.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_file(), format!("[update]\nmode = \"{text}\"\n")).unwrap();
        let env = Environment::from_process();

        let started = start(env, paths).unwrap();

        assert_eq!(started.settings.update_mode, expected, "{text}");
        started.core.shutdown().await;
    }
}

#[test]
fn the_event_source_hands_out_its_receiver_exactly_once() {
    let (_sender, receiver) = unbounded_channel::<Event>();
    let source = EventSource::new(receiver);
    let clone = source.clone();

    assert!(source.take().is_some());
    assert!(clone.take().is_none());
    assert!(source.take().is_none());
}

#[tokio::test]
async fn a_future_that_finishes_in_time_reports_success() {
    assert!(bounded(async {}, Duration::from_millis(200)).await);
}

#[tokio::test]
async fn a_core_that_never_answers_does_not_hold_the_exit_longer_than_the_bound() {
    let started = Instant::now();

    let finished = bounded(std::future::pending::<()>(), Duration::from_millis(60)).await;

    assert!(!finished);
    assert!(started.elapsed() < Duration::from_millis(1000));
}

#[tokio::test]
async fn shutting_down_sends_the_shutdown_command() {
    let (handle, mut commands): (CoreHandle, _) = CoreHandle::detached();

    shut_down(handle, Duration::from_millis(100)).await;

    assert!(matches!(commands.recv().await, Some(Command::Shutdown)));
}

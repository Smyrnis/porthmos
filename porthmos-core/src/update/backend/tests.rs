use std::sync::{Arc, atomic::AtomicBool};

use chrono::{TimeZone, Utc};

use super::*;
use crate::engine::testing::FakeUpdater;

fn info(version: &str) -> UpdateInfo {
    UpdateInfo {
        version: version.parse().unwrap(),
        released: Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap(),
        notes: String::new(),
        artifact: porthmos_update::Artifact {
            url: "https://example.com/a.tar.gz".to_string(),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
            size: 10,
        },
    }
}

#[test]
fn the_release_updater_builds_with_a_user_agent() {
    let dir = tempfile::tempdir().unwrap();

    assert!(ReleaseUpdater::new(dir.path().join("update"), "porthmos/9.9.9").is_ok());
}

#[tokio::test]
async fn the_trait_is_object_safe_and_usable_behind_an_arc() {
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Ok(Outcome::Available(info("0.5.0"))));
    let updater: Arc<dyn Updater> = Arc::new(fake);

    let outcome = updater.check(&"0.4.0".parse().unwrap()).await;

    assert!(matches!(outcome, Ok(Outcome::Available(found)) if found.version.to_string() == "0.5.0"));
}

#[tokio::test]
async fn the_fake_replays_its_progress_through_the_callback_and_records_its_calls() {
    let fake = Arc::new(FakeUpdater::default());
    *fake.progress.lock().unwrap() = vec![(1, 4), (2, 4), (4, 4)];
    fake.stages.lock().unwrap().push_back(Ok(Staged {
        version: "0.5.0".parse().unwrap(),
        binary: "/x".into(),
        sha256: "00".to_string(),
        size: 1,
    }));
    let mut seen = Vec::new();

    let staged = fake.stage(&info("0.5.0"), &AtomicBool::new(false), &mut |done, total| seen.push((done, total))).await;

    assert!(staged.is_ok());
    assert_eq!(seen, [(1, 4), (2, 4), (4, 4)]);
    assert_eq!(*fake.calls.lock().unwrap(), ["stage 0.5.0"]);
}

#[tokio::test]
async fn futures_of_the_updater_can_be_spawned_on_the_runtime() {
    let updater: Arc<dyn Updater> = Arc::new(FakeUpdater::default());

    let handle = tokio::spawn(async move { updater.check(&"0.4.0".parse().unwrap()).await });

    assert!(matches!(handle.await.unwrap(), Ok(Outcome::UpToDate)));
}

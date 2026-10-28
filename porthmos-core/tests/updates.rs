#![cfg(feature = "update")]

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use chrono::{TimeZone, Utc};
use porthmos_core::{
    ApplyOutcome, Command, Core, CoreHandle, Event, Paths, Severity, UpdateMode, UpdateSettings, UpdateState,
    apply_staged_update,
    config::Settings,
    update::{CheckError, Outcome, StageError, Staged, UpdateInfo, Updater, Version},
};
use porthmos_vfs::async_trait;
use tokio::sync::mpsc::UnboundedReceiver;

struct Script {
    checks: Mutex<VecDeque<Result<Outcome, CheckError>>>,
    stage_waits_for_cancel: bool,
    saw_cancel: AtomicBool,
    calls: Mutex<Vec<String>>,
}

impl Script {
    fn finding(version: &str) -> Arc<Script> {
        let script = Script::quiet();
        script.checks.lock().unwrap().push_back(Ok(Outcome::Available(info(version))));
        script
    }

    fn quiet() -> Arc<Script> {
        Arc::new(Script {
            checks: Mutex::new(VecDeque::new()),
            stage_waits_for_cancel: false,
            saw_cancel: AtomicBool::new(false),
            calls: Mutex::new(Vec::new()),
        })
    }

    fn waiting_for_cancel(version: &str) -> Arc<Script> {
        let script = Arc::new(Script {
            checks: Mutex::new(VecDeque::new()),
            stage_waits_for_cancel: true,
            saw_cancel: AtomicBool::new(false),
            calls: Mutex::new(Vec::new()),
        });
        script.checks.lock().unwrap().push_back(Ok(Outcome::Available(info(version))));
        script
    }
}

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

#[async_trait]
impl Updater for Script {
    async fn check(&self, _current: &Version) -> Result<Outcome, CheckError> {
        self.calls.lock().unwrap().push("check".to_string());
        self.checks.lock().unwrap().pop_front().unwrap_or(Ok(Outcome::UpToDate))
    }

    async fn stage(
        &self, update: &UpdateInfo, cancel: &AtomicBool, progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<Staged, StageError> {
        self.calls.lock().unwrap().push("stage".to_string());
        progress(5, 10);
        if self.stage_waits_for_cancel {
            while !cancel.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            self.saw_cancel.store(true, Ordering::Relaxed);
            return Err(StageError::Cancelled);
        }
        Ok(Staged { version: update.version.clone(), binary: "/x".into(), sha256: "00".to_string(), size: 1 })
    }
}

struct Started {
    core: CoreHandle,
    events: UnboundedReceiver<Event>,
    paths: Paths,
    _root: tempfile::TempDir,
}

fn start(mode: UpdateMode, updater: Arc<Script>) -> Started {
    let root = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(root.path());
    let settings = Settings { update: UpdateSettings { mode, interval_hours: 24 }, ..Settings::default() };
    let (core, events) = Core::builder()
        .without_keyring()
        .paths(paths.clone())
        .settings(settings)
        .local_home(root.path().to_path_buf())
        .updater(updater)
        .start()
        .unwrap();
    Started { core, events, paths, _root: root }
}

impl Started {
    async fn next<T>(&mut self, mut pick: impl FnMut(&Event) -> Option<T>) -> T {
        let mut seen = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(event)) => {
                    if let Some(value) = pick(&event) {
                        return value;
                    }
                    seen.push(format!("{event:?}"));
                }
                _ => panic!("expected event not received; saw: {seen:#?}"),
            }
        }
    }
}

#[tokio::test]
async fn a_started_core_announces_a_found_update_in_notify_mode() {
    let mut started = start(UpdateMode::Notify, Script::finding("9.9.9"));

    started.core.send(Command::CheckForUpdate { manual: true });
    let state = started
        .next(|event| match event {
            Event::Update(state) => Some(state.clone()),
            _ => None,
        })
        .await;

    assert_eq!(state, UpdateState::Available { version: "9.9.9".to_string() });
}

#[tokio::test]
async fn a_started_core_stages_and_announces_ready_in_auto_mode() {
    let script = Script::finding("9.9.9");
    let mut started = start(UpdateMode::Auto, script.clone());

    started.core.send(Command::CheckForUpdate { manual: true });
    let ready = started
        .next(|event| match event {
            Event::Update(UpdateState::Ready { version }) => Some(version.clone()),
            _ => None,
        })
        .await;

    assert_eq!(ready, "9.9.9");
    assert_eq!(*script.calls.lock().unwrap(), ["check", "stage"]);
}

#[tokio::test]
async fn a_started_core_with_updates_off_never_calls_the_updater() {
    let script = Script::finding("9.9.9");
    let mut started = start(UpdateMode::Off, script.clone());

    started.core.send(Command::CheckForUpdate { manual: true });
    let message = started
        .next(|event| match event {
            Event::Notice { severity: Severity::Warning, message } => Some(message.clone()),
            _ => None,
        })
        .await;

    assert_eq!(message, "Update checks are turned off in config.toml");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(script.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_manual_check_through_the_handle_reports_the_result() {
    let mut started = start(UpdateMode::Notify, Script::quiet());

    started.core.send(Command::CheckForUpdate { manual: true });
    let message = started
        .next(|event| match event {
            Event::Notice { message, .. } if message.contains("up to date") => Some(message.clone()),
            _ => None,
        })
        .await;

    assert_eq!(message, format!("Porthmos is up to date ({})", env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn shutdown_during_a_download_finishes_promptly() {
    let script = Script::waiting_for_cancel("9.9.9");
    let mut started = start(UpdateMode::Auto, script.clone());
    started.core.send(Command::CheckForUpdate { manual: true });
    started
        .next(|event| match event {
            Event::Update(UpdateState::Downloading { .. }) => Some(()),
            _ => None,
        })
        .await;

    tokio::time::timeout(Duration::from_secs(5), started.core.shutdown()).await.expect("shutdown finishes");

    assert!(script.saw_cancel.load(Ordering::Relaxed));
}

#[tokio::test]
async fn apply_staged_update_with_nothing_staged_is_nothing_after_shutdown() {
    let started = start(UpdateMode::Auto, Script::quiet());
    let paths = started.paths.clone();

    started.core.shutdown().await;

    assert_eq!(apply_staged_update(&paths, UpdateMode::Auto), ApplyOutcome::Nothing);
}

#[tokio::test]
async fn a_core_without_an_updater_and_without_release_updates_never_checks() {
    let root = tempfile::tempdir().unwrap();
    let (core, mut events) = Core::builder()
        .without_keyring()
        .paths(Paths::in_dir(root.path()))
        .local_home(root.path().to_path_buf())
        .start()
        .unwrap();

    core.send(Command::CheckForUpdate { manual: true });
    let message = loop {
        match tokio::time::timeout(Duration::from_secs(2), events.recv()).await.expect("an event").expect("open") {
            Event::Notice { message, .. } if message.contains("aren't available") => break message,
            _ => {}
        }
    };

    assert_eq!(message, "Update checks aren't available here");
}

#[tokio::test]
async fn asking_for_release_updates_builds_the_real_updater_without_contacting_anyone_at_startup() {
    let root = tempfile::tempdir().unwrap();

    let started = Core::builder()
        .without_keyring()
        .paths(Paths::in_dir(root.path()))
        .local_home(root.path().to_path_buf())
        .release_updates()
        .start();

    assert!(started.is_ok());
}

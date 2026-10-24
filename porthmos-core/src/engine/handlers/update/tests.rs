use std::{path::PathBuf, sync::Arc, time::Duration};

use chrono::{DateTime, Duration as Span, TimeZone, Utc};

use super::*;
use crate::{
    Severity,
    config::{UpdateMode, UpdateSettings},
    engine::{
        Command, Event,
        testing::{FakeUpdater, TestEngine, test_engine},
    },
    update::{CheckError, Outcome, UpdateInfo, UpdateState, UpdateTiming},
};

fn info(version: &str) -> UpdateInfo {
    UpdateInfo {
        version: version.parse().unwrap(),
        released: Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap(),
        notes: "notes".to_string(),
        artifact: porthmos_update::Artifact {
            url: "https://example.com/a.tar.gz".to_string(),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
            size: 10,
        },
    }
}

fn timing() -> UpdateTiming {
    UpdateTiming {
        startup_delay: Duration::from_millis(10),
        retry_after_failure: Duration::from_millis(40),
        interval: Duration::from_secs(60 * 60),
    }
}

struct Setup {
    t: TestEngine,
    fake: Arc<FakeUpdater>,
    seeded_check: Option<DateTime<Utc>>,
}

impl Setup {
    fn new(mode: UpdateMode) -> Self {
        Self::with(mode, FakeUpdater::default())
    }

    fn with(mode: UpdateMode, fake: FakeUpdater) -> Self {
        let mut t = test_engine();
        let fake = Arc::new(fake);
        let updater: Arc<dyn crate::update::Updater> = fake.clone();
        let record = crate::update::record::UpdateRecord::load(&t.engine.paths).0;
        t.engine.update = UpdateEngine::new(
            UpdateSettings { mode, interval_hours: 1 },
            record,
            Some(updater),
            timing(),
            Some(PathBuf::from("/nonexistent/porthmos")),
        );
        Setup { t, fake, seeded_check: None }
    }

    fn checked_at(mut self, at: DateTime<Utc>) -> Self {
        self.t.engine.update.record.set_last_checked(at);
        self.seeded_check = Some(at);
        self
    }

    fn calls(&self) -> Vec<String> {
        self.fake.calls.lock().unwrap().clone()
    }

    async fn pump(&mut self, times: usize) {
        for _ in 0..times {
            self.t.run_internal().await;
        }
    }

    async fn quiet_for(&mut self, millis: u64) {
        for _ in 0..2 {
            tokio::time::sleep(Duration::from_millis(millis)).await;
            while let Ok(done) = self.t.internal.try_recv() {
                self.t.engine.handle_internal(done);
            }
        }
    }

    fn stored_check(&self) -> Option<DateTime<Utc>> {
        self.t.engine.update.record.last_checked()
    }
}

fn updates(events: &[Event]) -> Vec<UpdateState> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Update(state) => Some(state.clone()),
            _ => None,
        })
        .collect()
}

fn notices(events: &[Event]) -> Vec<(Severity, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Notice { severity, message } => Some((*severity, message.clone())),
            _ => None,
        })
        .collect()
}

fn failure() -> CheckError {
    CheckError::Fetch("the request timed out".to_string())
}

#[tokio::test]
async fn an_automatic_check_runs_after_the_startup_delay_and_records_the_time() {
    let mut setup = Setup::new(UpdateMode::Notify);

    setup.t.engine.start_updates();
    assert!(setup.calls().is_empty());
    setup.pump(2).await;

    assert_eq!(setup.calls(), [format!("check {}", env!("CARGO_PKG_VERSION"))]);
    assert!(setup.stored_check().is_some());
    assert!(setup.t.drain().is_empty());
}

#[tokio::test]
async fn a_recent_check_is_not_repeated_at_startup() {
    let mut setup = Setup::new(UpdateMode::Notify).checked_at(Utc::now() - Span::minutes(10));

    setup.t.engine.start_updates();
    setup.quiet_for(150).await;

    assert!(setup.calls().is_empty());
}

#[tokio::test]
async fn an_old_check_runs_at_startup() {
    let mut setup = Setup::new(UpdateMode::Notify).checked_at(Utc::now() - Span::hours(48));

    setup.t.engine.start_updates();
    setup.pump(2).await;

    assert_eq!(setup.calls().len(), 1);
}

#[tokio::test]
async fn the_check_repeats_after_the_interval() {
    let mut setup = Setup::new(UpdateMode::Notify);
    setup.t.engine.update.timing.interval = Duration::from_millis(30);

    setup.t.engine.start_updates();
    setup.pump(4).await;

    assert_eq!(setup.calls().len(), 2);
}

#[tokio::test]
async fn a_failed_automatic_check_is_silent_and_not_recorded_and_is_retried() {
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Err(failure()));
    let mut setup = Setup::with(UpdateMode::Notify, fake);

    setup.t.engine.start_updates();
    setup.pump(2).await;

    assert!(setup.t.drain().is_empty());
    assert_eq!(setup.stored_check(), None);
    setup.pump(2).await;
    assert_eq!(setup.calls().len(), 2);
    assert!(setup.stored_check().is_some());
}

#[tokio::test]
async fn a_newer_version_in_notify_mode_announces_it_and_stages_nothing() {
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Ok(Outcome::Available(info("0.9.0"))));
    let mut setup = Setup::with(UpdateMode::Notify, fake);

    setup.t.engine.start_updates();
    setup.pump(2).await;

    assert_eq!(updates(&setup.t.drain()), [UpdateState::Available { version: "0.9.0".to_string() }]);
    assert!(!setup.calls().iter().any(|call| call.starts_with("stage")));
}

#[tokio::test]
async fn mode_off_never_calls_the_updater() {
    let mut setup = Setup::new(UpdateMode::Off);

    setup.t.engine.start_updates();
    setup.quiet_for(150).await;

    assert!(setup.calls().is_empty());
    assert!(setup.t.drain().is_empty());
}

#[tokio::test]
async fn a_manual_check_in_off_mode_says_it_is_off() {
    let mut setup = Setup::new(UpdateMode::Off);

    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });

    assert_eq!(
        notices(&setup.t.drain()),
        [(Severity::Warning, "Update checks are turned off in config.toml".to_string())]
    );
    assert!(setup.calls().is_empty());
}

#[tokio::test]
async fn a_manual_check_reports_up_to_date() {
    let mut setup = Setup::new(UpdateMode::Notify);

    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });
    setup.pump(1).await;

    assert_eq!(
        notices(&setup.t.drain()),
        [(Severity::Info, format!("Porthmos is up to date ({})", env!("CARGO_PKG_VERSION")))]
    );
    assert!(setup.stored_check().is_some());
}

#[tokio::test]
async fn a_manual_check_reports_a_failure() {
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Err(failure()));
    let mut setup = Setup::with(UpdateMode::Notify, fake);

    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });
    setup.pump(1).await;

    let found = notices(&setup.t.drain());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, Severity::Error);
    assert!(found[0].1.contains("timed out"), "{found:?}");
    assert_eq!(setup.stored_check(), None);
}

fn no_build() -> CheckError {
    CheckError::NoBuildForTarget { version: "0.9.0".parse().unwrap(), target: "x86_64-unknown-linux-gnu".to_string() }
}

#[tokio::test]
async fn a_manual_check_reports_no_build_for_this_platform() {
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Err(no_build()));
    let mut setup = Setup::with(UpdateMode::Notify, fake);

    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });
    setup.pump(1).await;

    let found = notices(&setup.t.drain());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, Severity::Warning);
    assert!(found[0].1.contains("0.9.0") && found[0].1.contains("x86_64-unknown-linux-gnu"), "{found:?}");
    assert!(setup.stored_check().is_some());
}

#[tokio::test]
async fn an_automatic_check_with_no_build_for_this_platform_is_silent() {
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Err(no_build()));
    let mut setup = Setup::with(UpdateMode::Notify, fake);

    setup.t.engine.start_updates();
    setup.pump(2).await;

    assert!(setup.t.drain().is_empty());
    assert!(setup.stored_check().is_some());
}

#[tokio::test]
async fn a_manual_check_while_one_is_running_says_so() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let fake = FakeUpdater { gate: Some(gate.clone()), ..FakeUpdater::default() };
    let mut setup = Setup::with(UpdateMode::Notify, fake);

    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });
    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });

    assert_eq!(notices(&setup.t.drain()), [(Severity::Info, "An update check is already running".to_string())]);
    gate.notify_one();
    setup.pump(1).await;
    assert_eq!(setup.calls().len(), 1);
}

#[tokio::test]
async fn a_package_managed_install_announces_managed_and_stages_nothing() {
    let home = tempfile::tempdir().unwrap();
    let home_path = std::fs::canonicalize(home.path()).unwrap();
    std::fs::create_dir_all(home_path.join(".cargo").join("bin")).unwrap();
    let executable = home_path.join(".cargo").join("bin").join("porthmos");
    std::fs::write(&executable, "x").unwrap();
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Ok(Outcome::Available(info("0.9.0"))));
    let mut setup = Setup::with(UpdateMode::Auto, fake);
    setup.t.engine.env.home = Some(home_path);
    setup.t.engine.update.executable = Some(executable);

    setup.t.engine.start_updates();
    setup.pump(2).await;

    let found = updates(&setup.t.drain());
    assert!(
        matches!(found.as_slice(), [UpdateState::Managed { version, reason }] if version == "0.9.0" && reason.contains(".cargo/bin")),
        "{found:?}"
    );
    assert!(!setup.calls().iter().any(|call| call.starts_with("stage")));
}

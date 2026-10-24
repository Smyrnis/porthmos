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

    fn updatable_executable(mut self) -> Self {
        let folder = self.t.dir.path().join("install");
        std::fs::create_dir_all(&folder).unwrap();
        let executable = folder.join("porthmos");
        std::fs::write(&executable, "current program").unwrap();
        self.t.engine.update.executable = Some(executable);
        self
    }

    fn failed_install(mut self, text: &str) -> Self {
        self.t.engine.update.record.set_last_failure(Some(text.to_string()));
        self
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

    async fn run_until(&mut self, done: impl Fn(&[Event]) -> bool) -> Vec<Event> {
        let mut seen = Vec::new();
        for _ in 0..400 {
            seen.extend(self.t.drain());
            if done(&seen) {
                return seen;
            }
            self.t.run_internal().await;
        }
        panic!("the expected events never arrived: {seen:?}");
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

fn staged_for(version: &str, binary: PathBuf) -> porthmos_update::Staged {
    porthmos_update::Staged { version: version.parse().unwrap(), binary, sha256: "00".to_string(), size: 1 }
}

fn seed_staged(setup: &Setup, version: &str) -> porthmos_update::Staged {
    let folder = setup.t.engine.paths.update_dir().join(version);
    std::fs::create_dir_all(&folder).unwrap();
    let binary = folder.join("porthmos");
    std::fs::write(&binary, b"x").unwrap();
    let staged = staged_for(version, binary);
    porthmos_update::record_staged(&porthmos_update::UpdateDir::new(setup.t.engine.paths.update_dir()), &staged)
        .unwrap();
    staged
}

fn ready(version: &str) -> UpdateState {
    UpdateState::Ready { version: version.to_string() }
}

fn auto_setup_finding(version: &str, stage: Result<porthmos_update::Staged, crate::update::StageError>) -> Setup {
    let fake = FakeUpdater::default();
    fake.checks.lock().unwrap().push_back(Ok(Outcome::Available(info(version))));
    fake.stages.lock().unwrap().push_back(stage);
    Setup::with(UpdateMode::Auto, fake).updatable_executable()
}

fn is_ready(events: &[Event]) -> bool {
    events.iter().any(|event| matches!(event, Event::Update(UpdateState::Ready { .. })))
}

#[tokio::test]
async fn auto_mode_stages_a_newer_version_and_announces_it_ready() {
    let mut setup = auto_setup_finding("0.9.0", Ok(staged_for("0.9.0", "/x".into())));
    *setup.fake.progress.lock().unwrap() = vec![(10, 100), (50, 100), (100, 100)];

    setup.t.engine.start_updates();
    let events = setup.run_until(is_ready).await;

    let states = updates(&events);
    assert_eq!(states.first(), Some(&UpdateState::Available { version: "0.9.0".to_string() }));
    assert_eq!(states.last(), Some(&ready("0.9.0")));
    assert!(states.iter().any(|state| matches!(state, UpdateState::Downloading { .. })), "{states:?}");
    assert_eq!(setup.calls().last().map(String::as_str), Some("stage 0.9.0"));
    assert!(!setup.t.engine.update.busy);
}

#[tokio::test]
async fn progress_is_reported_once_per_percent_at_most() {
    let mut setup = auto_setup_finding("0.9.0", Ok(staged_for("0.9.0", "/x".into())));
    *setup.fake.progress.lock().unwrap() = (1..=1000).map(|step| (step, 1000)).collect();

    setup.t.engine.start_updates();
    let events = setup.run_until(is_ready).await;

    let downloads: Vec<u64> = updates(&events)
        .into_iter()
        .filter_map(|state| match state {
            UpdateState::Downloading { downloaded, .. } => Some(downloaded),
            _ => None,
        })
        .collect();
    assert!(!downloads.is_empty() && downloads.len() <= 101, "{}", downloads.len());
    assert!(downloads.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[tokio::test]
async fn an_automatic_staging_failure_is_silent_and_retried_later() {
    let mut setup = auto_setup_finding("0.9.0", Err(crate::update::StageError::Download("boom".to_string())));

    setup.t.engine.start_updates();
    setup.pump(3).await;

    let events = setup.t.drain();
    assert!(notices(&events).is_empty(), "{events:?}");
    assert!(!updates(&events).contains(&ready("0.9.0")));
    assert!(!setup.t.engine.update.busy);
}

#[tokio::test]
async fn a_manual_staging_failure_is_reported() {
    let mut setup = auto_setup_finding("0.9.0", Err(crate::update::StageError::Download("boom".to_string())));

    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });
    let events = setup.run_until(|seen| notices(seen).iter().any(|(severity, _)| *severity == Severity::Error)).await;

    let errors: Vec<_> = notices(&events).into_iter().filter(|(severity, _)| *severity == Severity::Error).collect();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].1.contains("boom"), "{errors:?}");
}

#[tokio::test]
async fn startup_announces_an_already_staged_update_at_once_without_asking_the_updater() {
    let mut setup = Setup::new(UpdateMode::Auto);
    seed_staged(&setup, "0.9.0");

    setup.t.engine.start_updates();

    assert_eq!(updates(&setup.t.drain()), [ready("0.9.0")]);
    assert!(setup.calls().is_empty());
    assert!(setup.t.engine.update.staged.is_some());
}

#[tokio::test]
async fn startup_ignores_a_staged_update_in_notify_and_off_mode() {
    for mode in [UpdateMode::Notify, UpdateMode::Off] {
        let mut setup = Setup::new(mode);
        seed_staged(&setup, "0.9.0");

        setup.t.engine.start_updates();

        assert!(updates(&setup.t.drain()).is_empty(), "{mode:?}");
        assert!(setup.t.engine.paths.update_dir().join("0.9.0").join("porthmos").exists(), "{mode:?}");
        assert!(setup.t.engine.update.staged.is_none());
    }
}

#[tokio::test]
async fn startup_clears_a_staged_update_that_is_not_newer() {
    let mut setup = Setup::new(UpdateMode::Auto);
    seed_staged(&setup, "0.1.0");

    setup.t.engine.start_updates();

    assert!(updates(&setup.t.drain()).is_empty());
    assert!(!setup.t.engine.paths.update_dir().join("0.1.0").exists());
}

#[tokio::test]
async fn startup_removes_the_previous_version_file_only_in_auto_mode_on_an_updatable_install() {
    for (mode, removed) in [(UpdateMode::Auto, true), (UpdateMode::Notify, false), (UpdateMode::Off, false)] {
        let setup = Setup::new(mode).updatable_executable();
        let executable = setup.t.engine.update.executable.clone().unwrap();
        let old = executable.with_file_name("porthmos.old");
        std::fs::write(&old, "previous").unwrap();
        let mut setup = setup;

        setup.t.engine.start_updates();

        assert_eq!(!old.exists(), removed, "{mode:?}");
    }
}

#[tokio::test]
async fn startup_leaves_the_previous_version_file_of_a_package_managed_install_alone() {
    let home = tempfile::tempdir().unwrap();
    let home_path = std::fs::canonicalize(home.path()).unwrap();
    std::fs::create_dir_all(home_path.join(".cargo").join("bin")).unwrap();
    let executable = home_path.join(".cargo").join("bin").join("porthmos");
    std::fs::write(&executable, "x").unwrap();
    let old = executable.with_file_name("porthmos.old");
    std::fs::write(&old, "previous").unwrap();
    let mut setup = Setup::new(UpdateMode::Auto);
    setup.t.engine.env.home = Some(home_path);
    setup.t.engine.update.executable = Some(executable);

    setup.t.engine.start_updates();

    assert!(old.exists());
}

#[tokio::test]
async fn shutting_down_cancels_a_download_in_progress() {
    let fake = FakeUpdater { stage_waits_for_cancel: true, ..FakeUpdater::default() };
    fake.checks.lock().unwrap().push_back(Ok(Outcome::Available(info("0.9.0"))));
    let mut setup = Setup::with(UpdateMode::Auto, fake).updatable_executable();
    let saw_cancel = setup.fake.saw_cancel.clone();

    setup.t.engine.start_updates();
    setup.run_until(|events| !updates(events).is_empty()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    setup.t.engine.tasks.cancel_all();
    setup.pump(1).await;

    assert!(saw_cancel.load(std::sync::atomic::Ordering::Relaxed));
    let events = setup.t.drain();
    assert!(notices(&events).is_empty() && !updates(&events).contains(&ready("0.9.0")), "{events:?}");
}

#[tokio::test]
async fn the_second_check_does_not_stage_the_same_version_again() {
    let fake = FakeUpdater::default();
    for _ in 0..2 {
        fake.checks.lock().unwrap().push_back(Ok(Outcome::Available(info("0.9.0"))));
    }
    fake.stages.lock().unwrap().push_back(Ok(staged_for("0.9.0", "/x".into())));
    let mut setup = Setup::with(UpdateMode::Auto, fake).updatable_executable();

    setup.t.engine.start_updates();
    setup.run_until(is_ready).await;
    setup.t.engine.handle_command(Command::CheckForUpdate { manual: true });
    setup.pump(1).await;

    let stage_calls = setup.calls().iter().filter(|call| call.starts_with("stage")).count();
    assert_eq!(stage_calls, 1);
    assert!(!setup.t.engine.update.busy);
}

#[tokio::test]
async fn a_remembered_failure_is_reported_once_at_startup() {
    let mut setup = Setup::new(UpdateMode::Auto).failed_install("0.9.0: boom");

    setup.t.engine.start_updates();

    assert_eq!(
        notices(&setup.t.drain()),
        [(Severity::Warning, "Couldn't install the update: 0.9.0: boom".to_string())]
    );
    setup.t.engine.writer.flush().await;
    let saved = std::fs::read_to_string(setup.t.engine.paths.update_state_file()).unwrap_or_default();
    assert!(!saved.contains("boom"), "{saved}");
    setup.t.engine.start_updates();
    assert!(notices(&setup.t.drain()).is_empty());
}

#[tokio::test]
async fn a_remembered_failure_is_reported_in_notify_mode_too_but_not_in_off_mode() {
    let mut notify = Setup::new(UpdateMode::Notify).failed_install("0.9.0: boom");
    let mut off = Setup::new(UpdateMode::Off).failed_install("0.9.0: boom");

    notify.t.engine.start_updates();
    off.t.engine.start_updates();

    assert_eq!(notices(&notify.t.drain()).len(), 1);
    assert!(notices(&off.t.drain()).is_empty());
    assert_eq!(off.t.engine.update.record.last_failure(), Some("0.9.0: boom"));
}

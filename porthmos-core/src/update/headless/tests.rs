use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::{Mutex, atomic::AtomicBool},
    time::Duration,
};

use chrono::{TimeZone, Utc};
use porthmos_vfs::async_trait;

use super::*;
use crate::update::{
    CheckError, StageError, Staged,
    testing::{run_version, stage_on_disk, write_program},
};

const BUDGET: Duration = Duration::from_secs(5);

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

struct DiskUpdater {
    paths: Paths,
    check: Mutex<VecDeque<Result<Outcome, CheckError>>>,
    stage_error: Mutex<Option<StageError>>,
    calls: Mutex<Vec<String>>,
}

impl DiskUpdater {
    fn new(paths: &Paths, check: Result<Outcome, CheckError>) -> Self {
        DiskUpdater {
            paths: paths.clone(),
            check: Mutex::new(VecDeque::from([check])),
            stage_error: Mutex::new(None),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl Updater for DiskUpdater {
    async fn check(&self, _current: &Version) -> Result<Outcome, CheckError> {
        self.calls.lock().unwrap().push("check".to_string());
        self.check.lock().unwrap().pop_front().unwrap()
    }

    async fn stage(
        &self, update: &UpdateInfo, _cancel: &AtomicBool, _progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<Staged, StageError> {
        self.calls.lock().unwrap().push(format!("stage {}", update.version));
        if let Some(error) = self.stage_error.lock().unwrap().take() {
            return Err(error);
        }
        Ok(stage_on_disk(&self.paths, &update.version.to_string()))
    }
}

struct World {
    root: tempfile::TempDir,
    paths: Paths,
}

impl World {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(root.path());
        World { root, paths }
    }

    fn executable(&self) -> PathBuf {
        let path = self.root.path().join("bin").join("porthmos");
        if !path.exists() {
            write_program(&path, "0.3.0");
        }
        path
    }

    async fn run(&self, updater: &DiskUpdater, check_only: bool) -> Result<HeadlessUpdate, String> {
        let current: Version = "0.3.0".parse().unwrap();
        run_update_with(updater, &self.paths, &self.executable(), Some(self.root.path()), &current, check_only, BUDGET)
            .await
    }

    fn staged_files_exist(&self, version: &str) -> bool {
        self.paths.update_dir().join(version).join("porthmos").exists()
    }
}

#[tokio::test]
async fn up_to_date_reports_the_current_version() {
    let world = World::new();
    let updater = DiskUpdater::new(&world.paths, Ok(Outcome::UpToDate));

    let report = world.run(&updater, false).await;

    assert_eq!(report, Ok(HeadlessUpdate::UpToDate { current: "0.3.0".to_string() }));
    assert_eq!(updater.calls(), ["check"]);
}

#[tokio::test]
async fn check_only_reports_the_version_and_stages_nothing() {
    let world = World::new();
    let updater = DiskUpdater::new(&world.paths, Ok(Outcome::Available(info("0.9.0"))));

    let report = world.run(&updater, true).await;

    assert_eq!(report, Ok(HeadlessUpdate::Available { version: "0.9.0".to_string() }));
    assert_eq!(updater.calls(), ["check"]);
    assert!(!world.staged_files_exist("0.9.0"));
}

#[tokio::test]
async fn a_managed_executable_reports_the_reason_and_stages_nothing() {
    let world = World::new();
    let home = fs::canonicalize(world.root.path()).unwrap();
    let executable = home.join(".cargo").join("bin").join("porthmos");
    write_program(&executable, "0.3.0");
    let updater = DiskUpdater::new(&world.paths, Ok(Outcome::Available(info("0.9.0"))));
    let current: Version = "0.3.0".parse().unwrap();

    let report = run_update_with(&updater, &world.paths, &executable, Some(&home), &current, false, BUDGET).await;

    assert!(
        matches!(&report, Ok(HeadlessUpdate::Managed { version, reason }) if version == "0.9.0" && !reason.is_empty()),
        "{report:?}"
    );
    assert_eq!(updater.calls(), ["check"]);
}

#[tokio::test]
async fn an_available_update_is_staged_and_installed() {
    let world = World::new();
    let updater = DiskUpdater::new(&world.paths, Ok(Outcome::Available(info("0.9.0"))));

    let report = world.run(&updater, false).await;

    assert_eq!(report, Ok(HeadlessUpdate::Installed { version: "0.9.0".to_string() }));
    assert_eq!(updater.calls(), ["check", "stage 0.9.0"]);
    assert_eq!(run_version(&world.executable()), "porthmos 0.9.0");
}

#[tokio::test]
async fn an_already_staged_version_is_installed_without_staging_again() {
    let world = World::new();
    stage_on_disk(&world.paths, "0.9.0");
    let updater = DiskUpdater::new(&world.paths, Ok(Outcome::Available(info("0.9.0"))));

    let report = world.run(&updater, false).await;

    assert_eq!(report, Ok(HeadlessUpdate::Installed { version: "0.9.0".to_string() }));
    assert_eq!(updater.calls(), ["check"]);
    assert_eq!(run_version(&world.executable()), "porthmos 0.9.0");
}

#[tokio::test]
async fn a_failed_check_returns_the_error_text() {
    let world = World::new();
    let updater = DiskUpdater::new(&world.paths, Err(CheckError::Fetch("no route".to_string())));

    let report = world.run(&updater, false).await;

    assert_eq!(report, Err("couldn't check for updates: no route".to_string()));
}

#[tokio::test]
async fn a_failed_stage_returns_the_error_text_and_leaves_the_executable_alone() {
    let world = World::new();
    let updater = DiskUpdater::new(&world.paths, Ok(Outcome::Available(info("0.9.0"))));
    *updater.stage_error.lock().unwrap() = Some(StageError::Download("reset".to_string()));

    let report = world.run(&updater, false).await;

    assert_eq!(report, Err("couldn't download the update: reset".to_string()));
    assert_eq!(run_version(&world.executable()), "porthmos 0.3.0");
}

#[tokio::test]
async fn a_failed_install_returns_the_message_and_keeps_the_staged_files() {
    let world = World::new();
    let updater = DiskUpdater::new(&world.paths, Ok(Outcome::Available(info("0.9.0"))));
    let current: Version = "0.3.0".parse().unwrap();

    let report = run_update_with(
        &updater,
        &world.paths,
        &world.executable(),
        Some(world.root.path()),
        &current,
        false,
        Duration::ZERO,
    )
    .await;

    assert!(matches!(&report, Err(message) if message.contains("too long")), "{report:?}");
    assert!(world.staged_files_exist("0.9.0"));
    assert_eq!(run_version(&world.executable()), "porthmos 0.3.0");
}

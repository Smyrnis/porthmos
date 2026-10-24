use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;
use crate::update::{Version, record::UpdateRecord};

const BUDGET: Duration = Duration::from_secs(5);

fn script(version: &str) -> String {
    format!("#!/bin/sh\necho \"porthmos {version}\"\n")
}

fn write_program(path: &Path, version: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, script(version)).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn run_version(path: &Path) -> String {
    let mut attempts = 0;
    let output = loop {
        match std::process::Command::new(path).arg("--version").output() {
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 100 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            other => break other.unwrap(),
        }
    };
    String::from_utf8(output.stdout).unwrap().trim().to_string()
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
        self.root.path().join("bin").join("porthmos")
    }

    fn stage(&self, version: &str) {
        let binary = self.paths.update_dir().join(version).join("porthmos");
        write_program(&binary, version);
        let staged = porthmos_update::Staged {
            version: version.parse().unwrap(),
            binary: binary.clone(),
            sha256: porthmos_update::sha256_hex_of_file(&binary).unwrap(),
            size: fs::metadata(&binary).unwrap().len(),
        };
        porthmos_update::record_staged(&porthmos_update::UpdateDir::new(self.paths.update_dir()), &staged).unwrap();
    }

    fn apply(&self, mode: UpdateMode, budget: Duration) -> ApplyOutcome {
        let current: Version = "0.3.0".parse().unwrap();
        apply_with(&self.paths, mode, &self.executable(), Some(self.root.path()), &current, budget)
    }

    fn record(&self) -> UpdateRecord {
        UpdateRecord::load(&self.paths).0
    }

    fn staged_files_exist(&self, version: &str) -> bool {
        self.paths.update_dir().join(version).join("porthmos").exists()
    }
}

#[test]
fn a_staged_update_is_installed_and_reported() {
    let world = World::new();
    write_program(&world.executable(), "0.3.0");
    world.stage("0.9.0");

    let outcome = world.apply(UpdateMode::Auto, BUDGET);

    assert_eq!(outcome, ApplyOutcome::Installed { version: "0.9.0".to_string() });
    assert_eq!(run_version(&world.executable()), "porthmos 0.9.0");
    assert!(!world.staged_files_exist("0.9.0"));
}

#[test]
fn nothing_staged_is_nothing() {
    let world = World::new();
    write_program(&world.executable(), "0.3.0");

    assert_eq!(world.apply(UpdateMode::Auto, BUDGET), ApplyOutcome::Nothing);
    assert_eq!(run_version(&world.executable()), "porthmos 0.3.0");
}

#[test]
fn notify_and_off_modes_install_nothing_and_leave_everything() {
    for mode in [UpdateMode::Notify, UpdateMode::Off] {
        let world = World::new();
        write_program(&world.executable(), "0.3.0");
        world.stage("0.9.0");

        assert_eq!(world.apply(mode, BUDGET), ApplyOutcome::Nothing, "{mode:?}");
        assert_eq!(run_version(&world.executable()), "porthmos 0.3.0", "{mode:?}");
        assert!(world.staged_files_exist("0.9.0"), "{mode:?}");
    }
}

#[test]
fn a_package_managed_executable_is_left_alone() {
    let world = World::new();
    let home = fs::canonicalize(world.root.path()).unwrap();
    let executable = home.join(".cargo").join("bin").join("porthmos");
    write_program(&executable, "0.3.0");
    world.stage("0.9.0");
    let current: Version = "0.3.0".parse().unwrap();

    let outcome = apply_with(&world.paths, UpdateMode::Auto, &executable, Some(&home), &current, BUDGET);

    assert_eq!(outcome, ApplyOutcome::Nothing);
    assert_eq!(run_version(&executable), "porthmos 0.3.0");
    assert!(world.staged_files_exist("0.9.0"));
}

#[test]
fn an_executable_already_at_the_staged_version_is_nothing() {
    let world = World::new();
    write_program(&world.executable(), "0.9.0");
    world.stage("0.9.0");

    assert_eq!(world.apply(UpdateMode::Auto, BUDGET), ApplyOutcome::Nothing);
    assert!(!world.staged_files_exist("0.9.0"));
}

#[test]
fn an_install_that_fails_is_reported_and_remembered() {
    let world = World::new();
    write_program(&world.executable(), "0.3.0");
    world.stage("0.9.0");

    let outcome = world.apply(UpdateMode::Auto, Duration::ZERO);

    assert!(
        matches!(&outcome, ApplyOutcome::Failed { version, message } if version == "0.9.0" && message.contains("too long")),
        "{outcome:?}"
    );
    assert!(world.record().last_failure().is_some_and(|text| text.contains("0.9.0") && text.contains("too long")));
    assert_eq!(run_version(&world.executable()), "porthmos 0.3.0");
}

#[test]
fn a_later_success_clears_the_remembered_failure() {
    let world = World::new();
    write_program(&world.executable(), "0.3.0");
    world.stage("0.9.0");
    world.apply(UpdateMode::Auto, Duration::ZERO);

    let outcome = world.apply(UpdateMode::Auto, BUDGET);

    assert!(matches!(outcome, ApplyOutcome::Installed { .. }), "{outcome:?}");
    assert_eq!(world.record().last_failure(), None);
}

use std::{fs, path::PathBuf, time::Duration};

use super::*;
use crate::update::{
    AppIdentity, Product,
    record::UpdateRecord,
    testing::{run_version, stage_on_disk, stage_on_disk_for, staged_dir, write_program},
};

fn identity(product: Product, version: &str) -> AppIdentity {
    AppIdentity::new(product, version)
}

const BUDGET: Duration = Duration::from_secs(5);

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
        stage_on_disk(&self.paths, version);
    }

    fn apply(&self, mode: UpdateMode, budget: Duration) -> ApplyOutcome {
        apply_with(
            &self.paths,
            mode,
            &self.executable(),
            Some(self.root.path()),
            &identity(Product::Tui, "0.3.0"),
            budget,
        )
    }

    fn record(&self) -> UpdateRecord {
        UpdateRecord::load(&self.paths, "tui").0
    }

    fn staged_files_exist(&self, version: &str) -> bool {
        staged_dir(&self.paths, Product::Tui).path().join(version).join("porthmos").exists()
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
    let outcome =
        apply_with(&world.paths, UpdateMode::Auto, &executable, Some(&home), &identity(Product::Tui, "0.3.0"), BUDGET);

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

#[test]
fn a_staged_tui_update_is_not_installed_by_the_gui_and_stays_for_the_tui() {
    let world = World::new();
    let gui = world.root.path().join("bin").join("porthmos-gui");
    write_program(&gui, "2026.4.0");
    stage_on_disk_for(&world.paths, Product::Tui, "2026.5.0");

    let outcome = apply_with(
        &world.paths,
        UpdateMode::Auto,
        &gui,
        Some(world.root.path()),
        &identity(Product::Gui, "2026.4.0"),
        BUDGET,
    );

    assert_eq!(outcome, ApplyOutcome::Nothing);
    assert_eq!(run_version(&gui), "porthmos 2026.4.0");
    assert!(
        porthmos_update::find_staged(&staged_dir(&world.paths, Product::Tui), &"2026.4.0".parse().unwrap()).is_some()
    );
}

#[test]
fn a_staged_gui_update_is_installed_by_the_gui_with_its_own_failure_record() {
    let world = World::new();
    let gui = world.root.path().join("bin").join("porthmos-gui");
    write_program(&gui, "2026.4.0");
    stage_on_disk_for(&world.paths, Product::Gui, "2026.5.0");

    let outcome = apply_with(
        &world.paths,
        UpdateMode::Auto,
        &gui,
        Some(world.root.path()),
        &identity(Product::Gui, "2026.4.0"),
        BUDGET,
    );

    assert_eq!(outcome, ApplyOutcome::Installed { version: "2026.5.0".to_string() });
    assert_eq!(UpdateRecord::load(&world.paths, "tui").0.last_failure(), None);
}

use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;
use crate::{find_staged, sha256_hex_of_file, staged::record_staged};

const BUDGET: Duration = Duration::from_secs(5);

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

fn script(version_text: &str) -> String {
    format!("#!/bin/sh\necho \"porthmos {version_text}\"\n")
}

fn write_executable(path: &Path, body: &str, mode: u32) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

struct World {
    root: tempfile::TempDir,
}

impl World {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        World { root }
    }

    fn executable(&self) -> PathBuf {
        self.root.path().join("bin").join("porthmos")
    }

    fn install_running(&self, version_text: &str, mode: u32) {
        write_executable(&self.executable(), &script(version_text), mode);
    }

    fn dir(&self) -> UpdateDir {
        UpdateDir::new(self.root.path().join("update"))
    }

    fn stage(&self, version_text: &str) -> Staged {
        let folder = self.dir().path().join(version_text);
        fs::create_dir_all(&folder).unwrap();
        let binary = folder.join("porthmos");
        write_executable(&binary, &script(version_text), 0o755);
        let staged = Staged {
            version: version(version_text),
            binary: binary.clone(),
            sha256: sha256_hex_of_file(&binary).unwrap(),
            size: fs::metadata(&binary).unwrap().len(),
        };
        record_staged(&self.dir(), &staged).unwrap();
        staged
    }

    fn bin_listing(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.root.path().join("bin"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn run(&self, path: &Path) -> String {
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
}

fn running_as_root(folder: &Path) -> bool {
    let probe = folder.join(".root-probe");
    let created = fs::write(&probe, b"x").is_ok();
    let _ = fs::remove_file(&probe);
    created
}

#[test]
fn a_user_owned_folder_is_updatable() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);

    let place = install_place(&world.executable(), Some(world.root.path()));

    assert_eq!(place, InstallPlace::Updatable { executable: fs::canonicalize(world.executable()).unwrap() });
}

#[test]
fn a_path_under_a_package_manager_prefix_is_managed_with_the_prefix_in_the_reason() {
    let home = Path::new("/home/me");
    for (path, prefix) in [
        ("/usr/bin/porthmos", "/usr"),
        ("/usr/local/bin/porthmos", "/usr"),
        ("/opt/homebrew/bin/porthmos", "/opt/homebrew"),
        ("/home/linuxbrew/.linuxbrew/bin/porthmos", "/home/linuxbrew"),
        ("/nix/store/abc-porthmos/bin/porthmos", "/nix/store"),
        ("/snap/porthmos/1/bin/porthmos", "/snap"),
        ("/home/me/.cargo/bin/porthmos", ".cargo/bin"),
    ] {
        let reason = managed_reason(Path::new(path), Some(home)).unwrap_or_else(|| panic!("{path} was not managed"));

        assert!(reason.contains(prefix), "{path}: {reason}");
    }
}

#[test]
fn a_user_install_folder_is_not_managed() {
    let home = Path::new("/home/me");

    for path in ["/home/me/.local/share/porthmos/porthmos", "/home/me/bin/porthmos", "/srv/tools/porthmos"] {
        assert_eq!(managed_reason(Path::new(path), Some(home)), None, "{path}");
    }
}

#[test]
fn the_cargo_bin_folder_of_the_real_home_is_managed() {
    let world = World::new();
    let home = fs::canonicalize(world.root.path()).unwrap();
    fs::create_dir_all(home.join(".cargo").join("bin")).unwrap();
    let executable = home.join(".cargo").join("bin").join("porthmos");
    write_executable(&executable, &script("0.4.0"), 0o755);

    let place = install_place(&executable, Some(&home));

    assert!(matches!(&place, InstallPlace::Managed { reason } if reason.contains(".cargo/bin")), "{place:?}");
}

#[test]
fn an_unwritable_folder_is_managed() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let folder = world.root.path().join("bin");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    let skip = running_as_root(&folder);

    let place = install_place(&world.executable(), Some(world.root.path()));

    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    if skip {
        return;
    }
    assert!(matches!(&place, InstallPlace::Managed { reason } if reason.contains("writable")), "{place:?}");
}

#[test]
fn a_symlink_is_resolved_to_its_target() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let link = world.root.path().join("link");
    symlink(world.executable(), &link).unwrap();

    let place = install_place(&link, Some(world.root.path()));

    assert_eq!(place, InstallPlace::Updatable { executable: fs::canonicalize(world.executable()).unwrap() });
}

#[test]
fn the_new_binary_replaces_the_executable_and_keeps_its_permissions() {
    let world = World::new();
    world.install_running("0.4.0", 0o750);
    let staged = world.stage("0.5.0");

    let installed = install(&world.dir(), &staged, &world.executable(), BUDGET).unwrap();

    assert_eq!(installed.version, version("0.5.0"));
    assert_eq!(world.run(&world.executable()), "porthmos 0.5.0");
    assert_eq!(fs::metadata(world.executable()).unwrap().permissions().mode() & 0o777, 0o750);
}

#[test]
fn the_previous_version_is_kept_as_dot_old_and_still_runs() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");

    let installed = install(&world.dir(), &staged, &world.executable(), BUDGET).unwrap();

    let bin = fs::canonicalize(world.root.path().join("bin")).unwrap();
    assert_eq!(installed.previous, bin.join("porthmos.old"));
    assert_eq!(world.run(&installed.previous), "porthmos 0.4.0");
}

#[test]
fn an_earlier_dot_old_is_replaced_by_the_version_being_replaced() {
    let world = World::new();
    world.install_running("0.3.0", 0o755);
    let first = world.stage("0.4.0");
    install(&world.dir(), &first, &world.executable(), BUDGET).unwrap();
    let second = world.stage("0.5.0");

    let installed = install(&world.dir(), &second, &world.executable(), BUDGET).unwrap();

    assert_eq!(world.run(&installed.previous), "porthmos 0.4.0");
}

#[test]
fn the_staged_files_are_removed_after_installing() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");

    install(&world.dir(), &staged, &world.executable(), BUDGET).unwrap();

    assert_eq!(find_staged(&world.dir(), &version("0.4.0")), None);
    assert!(!world.dir().path().join("0.5.0").exists());
}

#[test]
fn a_staged_binary_changed_after_staging_is_discarded_and_not_installed() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");
    write_executable(&staged.binary, &script("6.6.6"), 0o755);

    let result = install(&world.dir(), &staged, &world.executable(), BUDGET);

    assert_eq!(result, Err(InstallError::Corrupt));
    assert_eq!(world.run(&world.executable()), "porthmos 0.4.0");
    assert!(!world.dir().path().join("0.5.0").exists());
}

#[test]
fn a_missing_staged_binary_is_nothing_staged() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");
    fs::remove_file(&staged.binary).unwrap();

    assert_eq!(install(&world.dir(), &staged, &world.executable(), BUDGET), Err(InstallError::NothingStaged));
}

#[test]
fn an_executable_already_at_or_above_the_staged_version_is_never_replaced() {
    for on_disk in ["0.5.0", "0.6.0"] {
        let world = World::new();
        world.install_running(on_disk, 0o755);
        let staged = world.stage("0.5.0");

        let result = install(&world.dir(), &staged, &world.executable(), BUDGET);

        assert_eq!(result, Err(InstallError::AlreadyCurrent), "{on_disk}");
        assert_eq!(world.run(&world.executable()), format!("porthmos {on_disk}"));
        assert_eq!(find_staged(&world.dir(), &version("0.4.0")), None);
    }
}

#[test]
fn a_second_install_after_the_first_finds_nothing_staged() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");
    install(&world.dir(), &staged, &world.executable(), BUDGET).unwrap();

    assert_eq!(install(&world.dir(), &staged, &world.executable(), BUDGET), Err(InstallError::NothingStaged));
}

#[test]
fn a_symlinked_executable_keeps_its_link_and_the_target_is_replaced() {
    let world = World::new();
    let target_folder = world.root.path().join("share");
    fs::create_dir(&target_folder).unwrap();
    let target = target_folder.join("porthmos");
    write_executable(&target, &script("0.4.0"), 0o755);
    let link = world.executable();
    symlink(&target, &link).unwrap();
    let staged = world.stage("0.5.0");

    install(&world.dir(), &staged, &link, BUDGET).unwrap();

    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_link(&link).unwrap(), target);
    assert_eq!(world.run(&target), "porthmos 0.5.0");
    assert_eq!(fs::read_dir(world.root.path().join("bin")).unwrap().count(), 1);
}

#[test]
fn an_unwritable_folder_changes_nothing_and_reports_not_writable() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");
    let folder = world.root.path().join("bin");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    let skip = running_as_root(&folder);

    let result = install(&world.dir(), &staged, &world.executable(), BUDGET);

    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    if skip {
        return;
    }
    assert!(matches!(result, Err(InstallError::NotWritable(_))), "{result:?}");
    assert_eq!(world.run(&world.executable()), "porthmos 0.4.0");
    assert_eq!(world.bin_listing(), ["porthmos"]);
}

#[test]
fn an_exhausted_budget_leaves_the_executable_untouched_and_no_temp_file() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");

    let result = install(&world.dir(), &staged, &world.executable(), Duration::ZERO);

    assert_eq!(result, Err(InstallError::TimedOut));
    assert_eq!(world.run(&world.executable()), "porthmos 0.4.0");
    assert_eq!(world.bin_listing(), ["porthmos"]);
    assert!(find_staged(&world.dir(), &version("0.4.0")).is_some());
}

#[test]
fn a_broken_executable_that_cannot_run_does_not_block_the_install() {
    let world = World::new();
    write_executable(&world.executable(), "not a program at all", 0o755);
    let staged = world.stage("0.5.0");

    install(&world.dir(), &staged, &world.executable(), BUDGET).unwrap();

    assert_eq!(world.run(&world.executable()), "porthmos 0.5.0");
}

#[test]
fn no_temporary_file_is_left_behind_after_a_success_either() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");

    install(&world.dir(), &staged, &world.executable(), BUDGET).unwrap();

    assert_eq!(world.bin_listing(), ["porthmos", "porthmos.old"]);
}

#[test]
fn forgetting_the_previous_version_removes_dot_old_and_ignores_a_missing_one() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");
    install(&world.dir(), &staged, &world.executable(), BUDGET).unwrap();

    forget_previous_version(&world.executable());
    forget_previous_version(&world.executable());

    assert_eq!(world.bin_listing(), ["porthmos"]);
}

#[test]
fn install_refuses_a_package_managed_location_even_when_the_folder_is_writable() {
    let world = World::new();
    let home = fs::canonicalize(world.root.path()).unwrap();
    fs::create_dir_all(home.join(".cargo").join("bin")).unwrap();
    let executable = home.join(".cargo").join("bin").join("porthmos");
    write_executable(&executable, &script("0.4.0"), 0o755);
    let staged = world.stage("0.5.0");

    let result = install_for(&world.dir(), &staged, &executable, BUDGET, Some(&home));

    assert!(matches!(&result, Err(InstallError::Managed(reason)) if reason.contains(".cargo/bin")), "{result:?}");
    assert_eq!(world.run(&executable), "porthmos 0.4.0");
    assert!(find_staged(&world.dir(), &version("0.4.0")).is_some());
    assert_eq!(fs::read_dir(executable.parent().unwrap()).unwrap().count(), 1);
}

#[test]
fn install_for_a_user_owned_location_still_installs() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let staged = world.stage("0.5.0");

    install_for(&world.dir(), &staged, &world.executable(), BUDGET, Some(world.root.path())).unwrap();

    assert_eq!(world.run(&world.executable()), "porthmos 0.5.0");
}

#[test]
fn many_simultaneous_location_checks_all_see_a_writable_folder() {
    let world = World::new();
    world.install_running("0.4.0", 0o755);
    let executable = world.executable();
    let home = world.root.path().to_path_buf();

    let results: Vec<InstallPlace> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..16)
            .map(|_| scope.spawn(|| (0..40).map(|_| install_place(&executable, Some(&home))).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|handle| handle.join().unwrap()).collect()
    });

    assert!(
        results.iter().all(|place| matches!(place, InstallPlace::Updatable { .. })),
        "{:?}",
        results.iter().find(|place| !matches!(place, InstallPlace::Updatable { .. }))
    );
    assert_eq!(world.bin_listing(), ["porthmos"]);
}

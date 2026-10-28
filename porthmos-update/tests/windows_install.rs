#![cfg(windows)]

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use porthmos_update::{
    Staged, UpdateDir, Version, forget_previous_version, install, record_staged, sha256_hex_of_file,
};

const STUB: &str = env!("CARGO_BIN_EXE_porthmos-update-stub");

fn stub_with_version(path: &Path, version: &str) {
    let mut bytes = fs::read(STUB).unwrap();
    bytes.extend_from_slice(format!("\nPORTHMOS-STUB-VERSION:{version}").as_bytes());
    fs::write(path, bytes).unwrap();
}

fn reported_version(program: &Path) -> String {
    let output = Command::new(program).arg("--version").output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
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
        self.root.path().join("bin").join("porthmos.exe")
    }

    fn dir(&self) -> UpdateDir {
        UpdateDir::new(self.root.path().join("update"))
    }

    fn stage(&self, version: &str) -> Staged {
        let folder = self.dir().path().join(version);
        fs::create_dir_all(&folder).unwrap();
        let binary = folder.join("porthmos.exe");
        stub_with_version(&binary, version);
        let staged = Staged {
            version: version.parse::<Version>().unwrap(),
            binary: binary.clone(),
            sha256: sha256_hex_of_file(&binary).unwrap(),
            size: fs::metadata(&binary).unwrap().len(),
        };
        record_staged(&self.dir(), &staged).unwrap();
        staged
    }
}

#[test]
fn installing_over_a_running_copy_replaces_it_and_keeps_the_old_one() {
    let world = World::new();
    stub_with_version(&world.executable(), "0.1.0");
    let mut running = Command::new(world.executable()).arg("--hold").stdin(Stdio::piped()).spawn().unwrap();
    let staged = world.stage("0.2.0");

    let installed = install(&world.dir(), &staged, &world.executable(), Duration::from_secs(10)).unwrap();

    assert!(running.try_wait().unwrap().is_none());
    assert_eq!(installed.version, "0.2.0".parse::<Version>().unwrap());
    assert_eq!(reported_version(&world.executable()), "porthmos 0.2.0");
    assert_eq!(reported_version(&installed.previous), "porthmos 0.1.0");
    running.stdin.take().unwrap().flush().unwrap();
    running.wait().unwrap();
    for _ in 0..50 {
        forget_previous_version(&world.executable());
        if !installed.previous.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!installed.previous.exists());
}

#[test]
fn a_staged_copy_that_is_not_newer_is_not_installed() {
    let world = World::new();
    stub_with_version(&world.executable(), "0.2.0");
    let staged = world.stage("0.2.0");

    let result = install(&world.dir(), &staged, &world.executable(), Duration::from_secs(10));

    assert!(matches!(result, Err(porthmos_update::InstallError::AlreadyCurrent)), "{result:?}");
    assert_eq!(reported_version(&world.executable()), "porthmos 0.2.0");
}

use std::{fs, path::Path};

use super::*;

const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

struct Fixture {
    root: tempfile::TempDir,
    dir: UpdateDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let dir = UpdateDir::new(root.path().join("update"));
        Fixture { root, dir }
    }

    fn stage(&self, version_text: &str, bytes: &[u8]) -> Staged {
        let folder = self.dir.path().join(version_text);
        fs::create_dir_all(&folder).unwrap();
        let binary = folder.join("porthmos");
        fs::write(&binary, bytes).unwrap();
        let staged =
            Staged { version: version(version_text), binary, sha256: HASH.to_string(), size: bytes.len() as u64 };
        record_staged(&self.dir, &staged).unwrap();
        staged
    }

    fn record_path(&self) -> PathBuf {
        self.dir.path().join("staged.toml")
    }

    fn names(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.dir.path()) else { return Vec::new() };
        let mut names: Vec<String> =
            entries.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
}

#[test]
fn a_recorded_update_is_found_again() {
    let fixture = Fixture::new();
    let staged = fixture.stage("0.5.0", b"binary bytes");

    assert_eq!(find_staged(&fixture.dir, &version("0.4.0")), Some(staged));
}

#[test]
fn an_update_that_is_not_newer_than_the_running_version_is_cleared() {
    for running in ["0.5.0", "0.6.0"] {
        let fixture = Fixture::new();
        fixture.stage("0.5.0", b"binary bytes");

        assert_eq!(find_staged(&fixture.dir, &version(running)), None, "{running}");
        assert!(fixture.names().is_empty(), "{running}: {:?}", fixture.names());
    }
}

#[test]
fn a_missing_binary_clears_the_record() {
    let fixture = Fixture::new();
    let staged = fixture.stage("0.5.0", b"binary bytes");
    fs::remove_file(&staged.binary).unwrap();

    assert_eq!(find_staged(&fixture.dir, &version("0.4.0")), None);
    assert!(!fixture.record_path().exists());
}

#[test]
fn a_binary_of_the_wrong_size_clears_the_record() {
    let fixture = Fixture::new();
    let staged = fixture.stage("0.5.0", b"binary bytes");
    fs::write(&staged.binary, b"short").unwrap();

    assert_eq!(find_staged(&fixture.dir, &version("0.4.0")), None);
    assert!(fixture.names().is_empty());
}

#[test]
fn a_corrupt_record_is_cleared_and_never_panics() {
    let outside = |fixture: &Fixture| {
        let path = fixture.root.path().join("outside");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("porthmos"), b"must survive").unwrap();
        path
    };
    let records = [
        "this is { not toml".to_string(),
        String::new(),
        format!("version = \"four\"\nbinary = \"4/porthmos\"\nsha256 = \"{HASH}\"\nsize = 1\n"),
        format!("version = \"0.5.0\"\nbinary = \"../outside/porthmos\"\nsha256 = \"{HASH}\"\nsize = 12\n"),
        format!("version = \"0.5.0\"\nbinary = \"/etc/passwd\"\nsha256 = \"{HASH}\"\nsize = 1\n"),
    ];
    for record in records {
        let fixture = Fixture::new();
        fixture.stage("0.5.0", b"binary bytes");
        let outside_folder = outside(&fixture);
        fs::write(fixture.record_path(), &record).unwrap();

        assert_eq!(find_staged(&fixture.dir, &version("0.4.0")), None, "{record}");
        assert!(fixture.names().is_empty(), "{record}: {:?}", fixture.names());
        assert_eq!(fs::read(outside_folder.join("porthmos")).unwrap(), b"must survive", "{record}");
    }
}

#[test]
fn nothing_staged_is_none_without_creating_anything() {
    let fixture = Fixture::new();

    assert_eq!(find_staged(&fixture.dir, &version("0.4.0")), None);
    assert!(!fixture.dir.path().exists());
}

#[test]
fn discard_removes_the_record_and_every_version_folder() {
    let fixture = Fixture::new();
    fixture.stage("0.5.0", b"one");
    fs::create_dir_all(fixture.dir.path().join("0.6.0")).unwrap();
    fs::create_dir_all(fixture.dir.path().join("download")).unwrap();

    discard_staged(&fixture.dir);

    assert!(fixture.names().is_empty(), "{:?}", fixture.names());
}

fn other_process_work(fixture: &Fixture) -> [PathBuf; 2] {
    let staging = fixture.dir.path().join("0.9.0.staging-4242");
    let download = fixture.dir.path().join("download.4242");
    for folder in [&staging, &download] {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join("partial"), b"in progress").unwrap();
    }
    [staging, download]
}

#[cfg(unix)]
fn open_directory(folder: &Path) -> fs::File {
    fs::File::open(folder).unwrap()
}

#[cfg(windows)]
fn open_directory(folder: &Path) -> fs::File {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
    fs::OpenOptions::new()
        .access_mode(FILE_WRITE_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(folder)
        .unwrap()
}

fn make_old(folder: &Path) {
    let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
    open_directory(folder).set_modified(two_hours_ago).unwrap();
}

#[test]
fn discarding_leaves_the_fresh_work_of_another_process_alone() {
    let fixture = Fixture::new();
    fixture.stage("0.5.0", b"one");
    let work = other_process_work(&fixture);

    discard_staged(&fixture.dir);

    for folder in &work {
        assert!(folder.join("partial").exists(), "{folder:?}");
    }
    assert!(!fixture.record_path().exists());
}

#[test]
fn discarding_removes_work_of_another_process_that_has_gone_stale() {
    let fixture = Fixture::new();
    fixture.stage("0.5.0", b"one");
    let work = other_process_work(&fixture);
    for folder in &work {
        make_old(folder);
    }

    discard_staged(&fixture.dir);

    assert!(fixture.names().is_empty(), "{:?}", fixture.names());
}

#[test]
fn discarding_when_nothing_exists_is_harmless() {
    let fixture = Fixture::new();

    discard_staged(&fixture.dir);
}

#[test]
fn recording_replaces_an_earlier_record_atomically() {
    let fixture = Fixture::new();
    fixture.stage("0.5.0", b"one");
    let second = fixture.stage("0.6.0", b"second bytes");

    assert_eq!(find_staged(&fixture.dir, &version("0.4.0")), Some(second));
    assert!(!fixture.names().iter().any(|name| name.ends_with(".tmp")), "{:?}", fixture.names());
}

use std::{fs, os::unix::fs::PermissionsExt};

use chrono::TimeZone;

use super::*;

fn paths() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    (dir, paths)
}

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 2, hour, 0, 0).unwrap()
}

#[test]
fn a_missing_file_loads_empty_without_a_warning() {
    let (_dir, paths) = paths();

    let (record, warning) = UpdateRecord::load(&paths);

    assert!(warning.is_none());
    assert_eq!(record.last_checked(), None);
    assert_eq!(record.last_failure(), None);
    assert!(record.is_writable());
    assert!(!paths.update_state_file().exists());
}

#[test]
fn a_saved_record_round_trips() {
    let (_dir, paths) = paths();
    let (mut record, _) = UpdateRecord::load(&paths);
    record.set_last_checked(at(9));
    record.set_last_failure(Some("0.5.0: couldn't write".to_string()));
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.update_state_file(), record.render().unwrap()).unwrap();

    let (again, warning) = UpdateRecord::load(&paths);

    assert!(warning.is_none());
    assert_eq!(again.last_checked(), Some(at(9)));
    assert_eq!(again.last_failure(), Some("0.5.0: couldn't write"));
}

#[test]
fn a_corrupt_file_is_set_aside_with_one_warning_and_loads_empty() {
    let (_dir, paths) = paths();
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.update_state_file(), "not toml at all [").unwrap();

    let (record, warning) = UpdateRecord::load(&paths);

    assert_eq!(record.last_checked(), None);
    assert!(record.is_writable());
    assert!(warning.unwrap().contains("update.toml.broken"));
    assert_eq!(
        fs::read_to_string(paths.update_state_file().with_extension("toml.broken")).unwrap(),
        "not toml at all ["
    );
}

#[test]
fn a_time_that_is_not_a_date_counts_as_corrupt() {
    let (_dir, paths) = paths();
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.update_state_file(), "last_checked = \"yesterday\"\n").unwrap();

    let (record, warning) = UpdateRecord::load(&paths);

    assert_eq!(record.last_checked(), None);
    assert!(warning.is_some());
}

#[test]
fn a_file_that_cannot_be_set_aside_is_not_writable_and_is_left_alone() {
    let (_dir, paths) = paths();
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.update_state_file(), "not toml at all [").unwrap();
    fs::set_permissions(&paths.state_dir, fs::Permissions::from_mode(0o500)).unwrap();
    if fs::File::create(paths.state_dir.join("probe")).is_ok() {
        fs::set_permissions(&paths.state_dir, fs::Permissions::from_mode(0o700)).unwrap();
        return;
    }

    let (record, warning) = UpdateRecord::load(&paths);
    fs::set_permissions(&paths.state_dir, fs::Permissions::from_mode(0o700)).unwrap();

    assert!(!record.is_writable());
    assert!(warning.is_some());
    assert_eq!(fs::read_to_string(paths.update_state_file()).unwrap(), "not toml at all [");
}

#[test]
fn unknown_keys_are_ignored() {
    let (_dir, paths) = paths();
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.update_state_file(), "future = 1\nlast_checked = \"2026-11-02T09:00:00Z\"\n").unwrap();

    let (record, warning) = UpdateRecord::load(&paths);

    assert!(warning.is_none());
    assert_eq!(record.last_checked(), Some(at(9)));
}

#[test]
fn the_failure_text_can_be_cleared() {
    let (_dir, paths) = paths();
    let (mut record, _) = UpdateRecord::load(&paths);
    record.set_last_failure(Some("x".to_string()));

    record.set_last_failure(None);

    assert_eq!(record.last_failure(), None);
    assert!(!record.render().unwrap().contains("last_failure"));
}

#[test]
fn an_empty_record_renders_to_an_empty_file() {
    let (_dir, paths) = paths();
    let (record, _) = UpdateRecord::load(&paths);

    assert_eq!(record.render().unwrap().trim(), "");
}

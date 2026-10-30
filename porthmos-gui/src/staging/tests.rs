use std::fs;
use std::time::Duration;

use porthmos_core::transfer::TransferSnapshot;
use porthmos_core::transfer::rows::{QueueRow, RowFailure, RowKind, RowState};

use super::{PREFIX, Staging, remove_stale};

fn row(kind: RowKind, state: RowState) -> QueueRow {
    QueueRow {
        kind,
        label: "file".to_string(),
        direction: porthmos_core::transfer::Direction::Download,
        files_done: 0,
        files_total: 1,
        bytes_done: 0,
        bytes_total: 100,
        state,
        paused: false,
        session_id: Some(1),
        failure: None,
    }
}

fn snapshot(rows: Vec<QueueRow>) -> TransferSnapshot {
    TransferSnapshot { rows, ..TransferSnapshot::default() }
}

fn staging_with(parent: &std::path::Path, names: &[&str]) -> Staging {
    Staging::create(parent).unwrap().expecting(names.iter().map(|name| name.to_string()).collect())
}

#[test]
fn staging_creates_a_private_folder_that_is_removed_on_remove() {
    let parent = tempfile::tempdir().unwrap();
    let staging = Staging::create(parent.path()).unwrap();
    let root = staging.root().to_path_buf();

    assert!(root.is_dir());
    assert!(root.starts_with(parent.path()));
    assert!(root.file_name().unwrap().to_string_lossy().starts_with(PREFIX));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&root).unwrap().permissions().mode() & 0o777, 0o700);
    }
    fs::create_dir(root.join("inner")).unwrap();
    fs::write(root.join("inner").join("file"), "data").unwrap();

    staging.remove();

    assert!(!root.exists());
}

#[test]
fn two_staging_folders_never_share_a_name() {
    let parent = tempfile::tempdir().unwrap();
    let first = Staging::create(parent.path()).unwrap();
    let second = Staging::create(parent.path()).unwrap();

    assert_ne!(first.root(), second.root());
}

#[test]
fn removing_twice_or_after_the_folder_vanished_does_not_panic() {
    let parent = tempfile::tempdir().unwrap();
    let staging = Staging::create(parent.path()).unwrap();
    fs::remove_dir(staging.root()).unwrap();

    staging.remove();
}

#[cfg(unix)]
#[test]
fn a_stale_staging_folder_older_than_a_day_is_removed_at_start_and_a_fresh_one_is_kept() {
    use std::time::SystemTime;

    let parent = tempfile::tempdir().unwrap();
    let stale = Staging::create(parent.path()).unwrap();
    let fresh = Staging::create(parent.path()).unwrap();
    let unrelated = parent.path().join("other-folder");
    let lookalike_file = parent.path().join(format!("{PREFIX}file"));
    fs::create_dir(&unrelated).unwrap();
    fs::write(&lookalike_file, "x").unwrap();
    let long_ago = SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
    for old in [stale.root(), unrelated.as_path(), lookalike_file.as_path()] {
        fs::File::open(old).unwrap().set_modified(long_ago).unwrap();
    }

    remove_stale(parent.path(), Duration::from_secs(24 * 3600));

    assert!(!stale.root().exists());
    assert!(fresh.root().exists());
    assert!(unrelated.exists());
    assert!(lookalike_file.exists());
}

#[test]
fn a_missing_parent_folder_is_not_an_error_for_the_stale_sweep() {
    let parent = tempfile::tempdir().unwrap();

    remove_stale(&parent.path().join("missing"), Duration::from_secs(1));
}

#[test]
fn finished_waits_until_a_row_was_seen_and_the_queue_is_idle() {
    let parent = tempfile::tempdir().unwrap();
    let staging = staging_with(parent.path(), &["a"]);
    fs::write(staging.root().join("a"), "x").unwrap();

    assert!(!staging.finished(&snapshot(vec![]), false));
    for busy in [RowState::Running, RowState::Queued, RowState::Scanning, RowState::AwaitingAnswer, RowState::Paused] {
        assert!(!staging.finished(&snapshot(vec![row(RowKind::Batch(1), busy)]), true), "{busy:?}");
    }
    assert!(staging.finished(&snapshot(vec![row(RowKind::Batch(1), RowState::Done)]), true));
}

#[test]
fn rows_already_in_the_queue_before_the_copy_do_not_hold_the_staging_back() {
    let parent = tempfile::tempdir().unwrap();
    let mut staging = staging_with(parent.path(), &["a"]);
    fs::write(staging.root().join("a"), "x").unwrap();
    staging.remember(&snapshot(vec![row(RowKind::Single(9), RowState::Running)]));

    let now = snapshot(vec![row(RowKind::Single(9), RowState::Running), row(RowKind::Batch(2), RowState::Done)]);

    assert!(staging.finished(&now, true));
    assert_eq!(staging.own_rows(&now).len(), 1);
}

#[test]
fn finished_needs_every_expected_name_present() {
    let parent = tempfile::tempdir().unwrap();
    let staging = staging_with(parent.path(), &["a", "b"]);
    let done = snapshot(vec![row(RowKind::Batch(1), RowState::Done)]);
    fs::write(staging.root().join("a"), "x").unwrap();

    assert!(!staging.finished(&done, true));

    fs::create_dir(staging.root().join("b")).unwrap();
    assert!(staging.finished(&done, true));
}

#[test]
fn a_failed_row_marks_the_staging_failed() {
    let parent = tempfile::tempdir().unwrap();
    let mut staging = staging_with(parent.path(), &["a"]);
    staging.remember(&snapshot(vec![row(RowKind::Single(9), RowState::Failed)]));
    let mut failed = row(RowKind::Batch(1), RowState::Failed);
    failed.failure = Some(RowFailure { file: "a".to_string(), message: "permission denied".to_string(), attempts: 3 });

    let broken = snapshot(vec![row(RowKind::Single(9), RowState::Failed), failed]);
    let message = staging.failure(&broken).expect("a failed row fails the staging");

    assert!(message.contains("permission denied"), "{message}");
    assert!(message.contains('a'), "{message}");
    assert!(staging.failure(&snapshot(vec![row(RowKind::Single(9), RowState::Failed)])).is_none());
    assert!(staging.failure(&snapshot(vec![row(RowKind::Batch(1), RowState::Done)])).is_none());
}

#[test]
fn a_partly_failed_or_cancelled_row_also_fails_the_staging() {
    let parent = tempfile::tempdir().unwrap();
    let staging = staging_with(parent.path(), &["a"]);

    for state in [RowState::PartlyFailed(2), RowState::Failed, RowState::Cancelled] {
        assert!(staging.failure(&snapshot(vec![row(RowKind::Batch(1), state)])).is_some(), "{state:?}");
    }
}

#[test]
fn progress_sums_only_the_stagings_own_rows() {
    let parent = tempfile::tempdir().unwrap();
    let mut staging = staging_with(parent.path(), &["a", "b"]);
    staging.remember(&snapshot(vec![row(RowKind::Single(9), RowState::Running)]));
    let mut first = row(RowKind::Single(1), RowState::Running);
    first.bytes_done = 50;
    let mut second = row(RowKind::Single(2), RowState::Running);
    second.bytes_done = 30;
    let mut old = row(RowKind::Single(9), RowState::Running);
    old.bytes_done = 100;

    assert_eq!(staging.progress(&snapshot(vec![old.clone(), first, second])), Some(40));
    assert_eq!(staging.progress(&snapshot(vec![old])), None);
}

#[test]
fn paths_are_the_expected_names_under_the_root_in_order() {
    let parent = tempfile::tempdir().unwrap();
    let staging = staging_with(parent.path(), &["z", "a"]);

    assert_eq!(staging.paths(), vec![staging.root().join("z"), staging.root().join("a")]);
}

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use porthmos_lfs::from_os_path;
use porthmos_vfs::{FileSystem, VfsPath, testing::FakeFs};

use super::*;
use crate::{
    Severity,
    engine::{
        Event, TransferEvent,
        testing::{TestEngine, test_engine},
    },
    transfer::{JobStatus, conflicts::ConflictPolicy, rows::RowState},
};

fn local_entry(t: &TestEngine, name: &str) -> Entry {
    let path = t.dir.path().join(name);
    let metadata = std::fs::metadata(&path).unwrap();
    Entry {
        modified: None,
        name: name.to_string(),
        path: from_os_path(&path),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        permissions: None,
        ..Default::default()
    }
}

async fn remote_entry(remote: &FakeFs, path: &str) -> Entry {
    let path = VfsPath::from(path);
    let metadata = remote.stat(&path).await.unwrap();
    Entry {
        modified: None,
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
        path,
        is_dir: metadata.is_dir(),
        size: metadata.size,
        permissions: None,
        ..Default::default()
    }
}

fn local(t: &TestEngine, name: &str) -> PathBuf {
    t.dir.path().join(name)
}

async fn upload(t: &mut TestEngine, session: u64, names: &[&str], dest: &str) {
    let entries = names.iter().map(|name| local_entry(t, name)).collect();
    t.engine.move_entries(Location::Local, entries, Location::Session(session), VfsPath::from(dest));
    t.settle().await;
}

async fn download(t: &mut TestEngine, session: u64, remote: &FakeFs, paths: &[&str]) {
    let mut entries = Vec::new();
    for path in paths {
        entries.push(remote_entry(remote, path).await);
    }
    t.engine.move_entries(Location::Session(session), entries, Location::Local, from_os_path(t.dir.path()));
    t.settle().await;
}

fn changed(events: &[Event], location: Location) -> bool {
    events.iter().any(|event| matches!(event, Event::LocationChanged { location: seen } if *seen == location))
}

fn only_row(t: &TestEngine) -> crate::transfer::rows::QueueRow {
    let rows = t.engine.transfers.rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    rows.into_iter().next().unwrap()
}

#[cfg(unix)]
fn is_root() -> bool {
    let probe = tempfile::tempdir().unwrap();
    let locked = probe.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(locked.join("f"), b"x").unwrap();
    std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o500)).unwrap();
    let removable = std::fs::remove_file(locked.join("f")).is_ok();
    let _ = std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o700));
    removable
}

#[tokio::test(flavor = "multi_thread")]
async fn an_uploaded_file_is_removed_from_this_computer_once_it_is_on_the_server() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::write(local(&t, "a.txt"), b"hello").unwrap();

    upload(&mut t, session, &["a.txt"], "/up").await;

    assert_eq!(remote.contents("/up/a.txt").unwrap(), b"hello");
    assert!(!local(&t, "a.txt").exists());
    assert_eq!(only_row(&t).state, RowState::Done);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_downloaded_file_is_removed_from_the_server_once_it_is_here() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a.txt", b"hello", None);

    download(&mut t, session, &remote, &["/srv/a.txt"]).await;

    assert_eq!(std::fs::read(local(&t, "a.txt")).unwrap(), b"hello");
    assert!(!remote.exists("/srv/a.txt"));
    assert_eq!(only_row(&t).state, RowState::Done);
}

#[tokio::test(flavor = "multi_thread")]
async fn moving_a_folder_leaves_nothing_behind_once_everything_arrived() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::create_dir_all(local(&t, "tree/inner/deep")).unwrap();
    std::fs::write(local(&t, "tree/a.txt"), b"a").unwrap();
    std::fs::write(local(&t, "tree/inner/b.txt"), b"b").unwrap();

    upload(&mut t, session, &["tree"], "/up").await;

    assert_eq!(remote.contents("/up/tree/a.txt").unwrap(), b"a");
    assert_eq!(remote.contents("/up/tree/inner/b.txt").unwrap(), b"b");
    assert!(remote.exists("/up/tree/inner/deep"));
    assert!(!local(&t, "tree").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_downloaded_folder_is_removed_from_the_server_when_it_is_empty() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/tree/a.txt", b"a", None).file("/srv/tree/inner/b.txt", b"b", None).dir("/srv/tree/empty");

    download(&mut t, session, &remote, &["/srv/tree"]).await;

    assert_eq!(std::fs::read(local(&t, "tree/inner/b.txt")).unwrap(), b"b");
    assert!(local(&t, "tree/empty").is_dir());
    assert!(!remote.exists("/srv/tree"));
    assert!(remote.exists("/srv"));
}

#[tokio::test(flavor = "multi_thread")]
async fn moving_a_folder_leaves_a_sibling_with_the_same_prefix_alone() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a/one.txt", b"1", None).file("/srv/ab/two.txt", b"2", None);

    download(&mut t, session, &remote, &["/srv/a"]).await;

    assert_eq!(std::fs::read(local(&t, "a/one.txt")).unwrap(), b"1");
    assert!(!remote.exists("/srv/a"));
    assert_eq!(remote.contents("/srv/ab/two.txt").unwrap(), b"2");
    assert!(!local(&t, "ab").exists());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_remote_name_with_a_backslash_is_moved_as_one_file() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a\\b.txt", b"odd", None);

    download(&mut t, session, &remote, &["/srv/a\\b.txt"]).await;

    assert_eq!(std::fs::read(local(&t, "a\\b.txt")).unwrap(), b"odd");
    assert!(!remote.exists("/srv/a\\b.txt"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_skipped_because_it_already_exists_stays_and_keeps_its_folder() {
    let mut t = test_engine();
    t.engine.on_conflict = ConflictPolicy::Skip;
    let (session, remote) = t.add_session("srv");
    remote.file("/up/tree/old.txt", b"theirs", None);
    std::fs::create_dir(local(&t, "tree")).unwrap();
    std::fs::write(local(&t, "tree/old.txt"), b"mine").unwrap();
    std::fs::write(local(&t, "tree/new.txt"), b"new").unwrap();

    upload(&mut t, session, &["tree"], "/up").await;

    assert_eq!(remote.contents("/up/tree/old.txt").unwrap(), b"theirs");
    assert_eq!(remote.contents("/up/tree/new.txt").unwrap(), b"new");
    assert_eq!(std::fs::read(local(&t, "tree/old.txt")).unwrap(), b"mine");
    assert!(!local(&t, "tree/new.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn overwriting_on_conflict_moves_the_file_over_the_existing_one() {
    let mut t = test_engine();
    t.engine.on_conflict = ConflictPolicy::Overwrite;
    let (session, remote) = t.add_session("srv");
    remote.file("/up/a.txt", b"old", None);
    std::fs::write(local(&t, "a.txt"), b"fresh").unwrap();

    upload(&mut t, session, &["a.txt"], "/up").await;

    assert_eq!(remote.contents("/up/a.txt").unwrap(), b"fresh");
    assert!(!local(&t, "a.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_copy_keeps_the_original() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up").fail_shutdown("/up/a.txt.part");
    std::fs::write(local(&t, "a.txt"), b"hello").unwrap();

    upload(&mut t, session, &["a.txt"], "/up").await;

    assert!(local(&t, "a.txt").exists());
    assert!(t.engine.transfers.jobs().all(|job| matches!(job.status, JobStatus::Failed(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_file_keeps_its_folder_while_the_others_are_moved() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up").fail_shutdown("/up/tree/bad.txt.part");
    std::fs::create_dir(local(&t, "tree")).unwrap();
    std::fs::write(local(&t, "tree/good.txt"), b"g").unwrap();
    std::fs::write(local(&t, "tree/bad.txt"), b"b").unwrap();

    upload(&mut t, session, &["tree"], "/up").await;

    assert!(!local(&t, "tree/good.txt").exists());
    assert!(local(&t, "tree/bad.txt").exists());
    assert_eq!(remote.contents("/up/tree/good.txt").unwrap(), b"g");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_symlink_inside_a_folder_is_never_moved_and_keeps_the_folder() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::create_dir(local(&t, "tree")).unwrap();
    std::fs::write(local(&t, "tree/a.txt"), b"a").unwrap();
    std::os::unix::fs::symlink(local(&t, "tree/a.txt"), local(&t, "tree/link")).unwrap();

    upload(&mut t, session, &["tree"], "/up").await;

    assert!(!local(&t, "tree/a.txt").exists());
    assert!(local(&t, "tree/link").symlink_metadata().is_ok());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_source_that_cannot_be_removed_fails_the_job_and_keeps_the_copy() {
    if is_root() {
        return;
    }
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::create_dir(local(&t, "sealed")).unwrap();
    std::fs::write(local(&t, "sealed/a.txt"), b"hello").unwrap();
    std::fs::set_permissions(local(&t, "sealed"), std::os::unix::fs::PermissionsExt::from_mode(0o500)).unwrap();
    let entry = Entry {
        modified: None,
        name: "a.txt".to_string(),
        path: from_os_path(&local(&t, "sealed/a.txt")),
        is_dir: false,
        size: 5,
        permissions: None,
        ..Default::default()
    };

    t.engine.move_entries(Location::Local, vec![entry], Location::Session(session), VfsPath::from("/up"));
    t.settle().await;
    std::fs::set_permissions(local(&t, "sealed"), std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();

    assert_eq!(remote.contents("/up/a.txt").unwrap(), b"hello");
    assert!(local(&t, "sealed/a.txt").exists());
    let job = t.engine.transfers.jobs().next().unwrap();
    assert!(
        matches!(&job.status, JobStatus::Failed(message) if message.contains("couldn't be removed")),
        "{:?}",
        job.status
    );
    assert!(matches!(only_row(&t).state, RowState::Failed | RowState::PartlyFailed(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_row_is_labelled_as_a_move() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(local(&t, name), b"x").unwrap();
    }

    upload(&mut t, session, &["a.txt"], "/up").await;
    assert_eq!(only_row(&t).label, "a.txt (move)");

    t.engine.handle_command(crate::Command::ClearFinished);
    upload(&mut t, session, &["b.txt", "c.txt"], "/up").await;
    assert_eq!(only_row(&t).label, "2 items (move)");
}

#[tokio::test(flavor = "multi_thread")]
async fn both_panels_are_refreshed_when_a_move_finishes() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::write(local(&t, "a.txt"), b"a").unwrap();
    t.drain();

    upload(&mut t, session, &["a.txt"], "/up").await;

    let events = t.drain();
    assert!(changed(&events, Location::Local), "{events:?}");
    assert!(changed(&events, Location::Session(session)), "{events:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plain_copy_still_keeps_the_original() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::write(local(&t, "a.txt"), b"a").unwrap();
    let entry = local_entry(&t, "a.txt");

    t.engine.copy(Location::Local, vec![entry], Location::Session(session), VfsPath::from("/up"));
    t.settle().await;

    assert!(local(&t, "a.txt").exists());
    assert_eq!(only_row(&t).label, "a.txt");
}

#[test]
fn moving_between_two_sessions_is_refused() {
    let mut t = test_engine();

    t.engine.move_entries(Location::Session(1), vec![], Location::Session(2), VfsPath::from("/"));

    assert_eq!(
        t.first_notice(),
        Some((Severity::Warning, "Moving between two remote sessions isn't supported yet".to_string()))
    );
}

#[test]
fn moving_nothing_does_nothing() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");

    t.engine.move_entries(Location::Local, vec![], Location::Session(session), VfsPath::from("/up"));

    assert!(t.engine.planning.is_empty());
    assert!(t.drain().is_empty());
}

#[test]
fn moving_to_a_disconnected_session_reports_it() {
    let mut t = test_engine();
    std::fs::write(local(&t, "a.txt"), b"a").unwrap();
    let entry = local_entry(&t, "a.txt");

    t.engine.move_entries(Location::Local, vec![entry], Location::Session(9), VfsPath::from("/up"));

    assert_eq!(t.first_notice(), Some((Severity::Error, "Move failed: session disconnected".to_string())));
    assert!(local(&t, "a.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn clearing_finished_rows_forgets_the_move() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::write(local(&t, "a.txt"), b"a").unwrap();
    upload(&mut t, session, &["a.txt"], "/up").await;
    assert_eq!(t.engine.moves.len(), 1);

    t.engine.handle_command(crate::Command::ClearFinished);

    assert!(t.engine.moves.is_empty());
}

#[tokio::test]
async fn the_source_is_removed_only_when_the_copy_matches() {
    let source = FakeFs::new();
    let destination = FakeFs::new();
    source.file("/s/a.txt", b"hello", None);
    destination.file("/d/a.txt", b"hello", None);

    remove_copied_source(&source, &VfsPath::new("/s/a.txt"), &destination, &VfsPath::new("/d/a.txt")).await.unwrap();

    assert!(!source.exists("/s/a.txt"));
    assert!(destination.exists("/d/a.txt"));
}

#[tokio::test]
async fn a_copy_of_another_size_keeps_the_source() {
    let source = FakeFs::new();
    let destination = FakeFs::new();
    source.file("/s/a.txt", b"hello world", None);
    destination.file("/d/a.txt", b"hello", None);

    let error = remove_copied_source(&source, &VfsPath::new("/s/a.txt"), &destination, &VfsPath::new("/d/a.txt"))
        .await
        .unwrap_err();

    assert!(error.contains("doesn't match"), "{error}");
    assert!(source.exists("/s/a.txt"));
}

#[tokio::test]
async fn a_missing_copy_keeps_the_source() {
    let source = FakeFs::new();
    let destination = FakeFs::new();
    source.file("/s/a.txt", b"hello", None);

    assert!(
        remove_copied_source(&source, &VfsPath::new("/s/a.txt"), &destination, &VfsPath::new("/d/a.txt"))
            .await
            .is_err()
    );

    assert!(source.exists("/s/a.txt"));
}

#[tokio::test]
async fn a_source_that_is_already_gone_counts_as_removed() {
    let source = FakeFs::new();
    let destination = FakeFs::new();
    destination.file("/d/a.txt", b"hello", None);

    remove_copied_source(&source, &VfsPath::new("/s/a.txt"), &destination, &VfsPath::new("/d/a.txt")).await.unwrap();
}

#[tokio::test]
async fn emptied_folders_are_removed_deepest_first_and_only_when_empty() {
    let fs = FakeFs::new();
    fs.dir("/r/a/b").dir("/r/a/c").file("/r/keep/x.txt", b"x", None).dir("/r/keep/empty");

    let removed = remove_emptied_directories(&fs, &[VfsPath::from("/r/a"), VfsPath::from("/r/keep")]).await;

    assert_eq!(removed, 4);
    assert!(!fs.exists("/r/a"));
    assert!(fs.exists("/r/keep/x.txt") && fs.exists("/r/keep"));
    assert!(!fs.exists("/r/keep/empty"));
}

#[tokio::test]
async fn a_root_that_is_gone_is_not_an_error() {
    let fs = FakeFs::new();

    assert_eq!(remove_emptied_directories(&fs, &[VfsPath::from("/nope")]).await, 0);
}

#[test]
fn a_cancelled_move_scan_says_the_move_was_cancelled() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    let batch_id = t.engine.transfers.start_batch("a.txt (move)".to_string());
    t.engine.moves.insert(batch_id, MoveBatch { session_id: session, direction: Direction::Upload, roots: Vec::new() });

    t.engine.handle_transfer_event(TransferEvent::PlanCancelled {
        batch_id,
        session_id: session,
        direction: Direction::Upload,
    });

    assert_eq!(t.first_notice(), Some((Severity::Info, "Move cancelled".to_string())));
}

#[test]
fn a_cancelled_copy_scan_still_says_the_copy_was_cancelled() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    let batch_id = t.engine.transfers.start_batch("a.txt".to_string());

    t.engine.handle_transfer_event(TransferEvent::PlanCancelled {
        batch_id,
        session_id: session,
        direction: Direction::Upload,
    });

    assert_eq!(t.first_notice(), Some((Severity::Info, "Copy cancelled".to_string())));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_history_remembers_a_move_under_its_move_label() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::write(local(&t, "a.txt"), b"a").unwrap();
    upload(&mut t, session, &["a.txt"], "/up").await;

    t.engine.publish_transfers();

    let labels: Vec<&str> = t.engine.history.store.entries().iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(labels, ["a.txt (move)"]);
}

fn set_modified(path: &Path, seconds_ago: u64) {
    let when = std::time::SystemTime::now() - std::time::Duration::from_secs(seconds_ago);
    std::fs::OpenOptions::new().write(true).open(path).unwrap().set_modified(when).unwrap();
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn planned(source: &Path, destination: &str, size: u64) -> crate::transfer::plan::PlannedFile {
    crate::transfer::plan::PlannedFile {
        source: from_os_path(source),
        destination: VfsPath::from(destination),
        display_name: VfsPath::from(destination).file_name().unwrap().to_string_lossy().into_owned(),
        size,
        existing: None,
        source_modified: None,
        partial: None,
        resume: false,
    }
}

fn plan_of(files: Vec<crate::transfer::plan::PlannedFile>) -> crate::transfer::plan::DirectoryPlan {
    crate::transfer::plan::DirectoryPlan { files, skipped_symlinks: 0, taken_names: std::collections::HashMap::new() }
}

fn move_batch(t: &mut TestEngine, session: u64, roots: Vec<VfsPath>) -> u64 {
    let batch_id = t.engine.transfers.start_batch("batch (move)".to_string());
    t.engine.moves.insert(batch_id, MoveBatch { session_id: session, direction: Direction::Upload, roots });
    batch_id
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_partial_copy_is_never_resumed_so_the_original_is_never_lost_to_a_spliced_copy() {
    let mut t = test_engine();
    t.engine.on_conflict = ConflictPolicy::Skip;
    let (session, remote) = t.add_session("srv");
    remote.file("/up/a.txt.part", b"OLDOLD", Some(now_seconds()));
    std::fs::write(local(&t, "a.txt"), b"newnewnewnew").unwrap();
    set_modified(&local(&t, "a.txt"), 3600);

    upload(&mut t, session, &["a.txt"], "/up").await;

    assert_eq!(remote.contents("/up/a.txt").unwrap(), b"newnewnewnew");
    assert!(!local(&t, "a.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retried_move_starts_the_copy_again_from_the_beginning() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/up/a.txt.part", b"STALE", Some(now_seconds()));
    std::fs::write(local(&t, "a.txt"), b"hello").unwrap();
    set_modified(&local(&t, "a.txt"), 3600);
    let batch_id = move_batch(&mut t, session, Vec::new());
    let id = t.engine.transfers.enqueue(
        session,
        Direction::Upload,
        from_os_path(&local(&t, "a.txt")),
        "/up/a.txt".to_string(),
        "a.txt".to_string(),
        5,
        Some(batch_id),
    );
    {
        let mut job = t.engine.transfers.get_mut(id).unwrap();
        job.remove_source = true;
        job.resume = true;
    }

    t.engine.fill_transfer_slots();
    t.settle().await;

    assert_eq!(remote.contents("/up/a.txt").unwrap(), b"hello");
    assert!(!local(&t, "a.txt").exists());
}

#[test]
fn only_loopback_hosts_are_recognised_as_this_computer() {
    for host in ["localhost", "LOCALHOST", "127.0.0.1", "127.1.2.3", "::1", "[::1]", "sub.localhost"] {
        assert!(is_loopback(host), "{host}");
    }
    for host in ["example.com", "192.168.0.2", "10.0.0.1", "localhost.example.com", "", "::2"] {
        assert!(!is_loopback(host), "{host}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_is_never_moved_onto_itself_through_a_loopback_session() {
    let mut t = test_engine();
    t.engine.on_conflict = ConflictPolicy::Overwrite;
    let same_place = Arc::new(porthmos_lfs::LocalFs::new(t.dir.path().to_path_buf()));
    let session = t.add_session_with("self", same_place);
    t.engine.sessions.get_mut(&session).unwrap().entry.host = "localhost".to_string();
    std::fs::write(local(&t, "a.txt"), b"precious").unwrap();

    let dest = t.dir.path().to_str().unwrap().to_string();
    upload(&mut t, session, &["a.txt"], &dest).await;

    assert_eq!(std::fs::read(local(&t, "a.txt")).unwrap(), b"precious");
    assert_eq!(t.engine.transfers.jobs().count(), 0);
    let notices = t.notices();
    assert!(
        notices.iter().any(|(severity, message)| *severity == Severity::Warning && message.contains("onto itself")),
        "{notices:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_loopback_session_still_moves_files_to_a_different_place() {
    let mut t = test_engine();
    let elsewhere = tempfile::tempdir().unwrap();
    let place = Arc::new(porthmos_lfs::LocalFs::new(elsewhere.path().to_path_buf()));
    let session = t.add_session_with("self", place);
    t.engine.sessions.get_mut(&session).unwrap().entry.host = "127.0.0.1".to_string();
    std::fs::write(local(&t, "a.txt"), b"hello").unwrap();

    upload(&mut t, session, &["a.txt"], elsewhere.path().to_str().unwrap()).await;

    assert_eq!(std::fs::read(elsewhere.path().join("a.txt")).unwrap(), b"hello");
    assert!(!local(&t, "a.txt").exists());
}

#[test]
fn names_that_differ_only_by_case_keep_their_originals_and_warn() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    let batch_id = move_batch(&mut t, session, Vec::new());
    let plan = plan_of(vec![
        planned(&local(&t, "A.txt"), "/up/A.txt", 1),
        planned(&local(&t, "a.txt"), "/up/a.txt", 2),
        planned(&local(&t, "other.txt"), "/up/other.txt", 3),
    ]);
    t.engine.max_parallel = 0;

    t.engine.apply_plan_ready(batch_id, session, Direction::Upload, plan, &[], false);

    let flags: Vec<(String, bool)> =
        t.engine.transfers.jobs().map(|job| (job.display_name.clone(), job.remove_source)).collect();
    assert_eq!(flags.len(), 3);
    for (name, removes) in flags {
        assert_eq!(removes, name == "other.txt", "{name}");
    }
    let notices = t.notices();
    assert!(
        notices
            .iter()
            .any(|(severity, message)| *severity == Severity::Warning && message.contains("differ only by case")),
        "{notices:?}"
    );
}

#[test]
fn the_same_name_in_different_folders_is_not_a_case_collision() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    let batch_id = move_batch(&mut t, session, Vec::new());
    let plan = plan_of(vec![
        planned(&local(&t, "x/a.txt"), "/up/x/a.txt", 1),
        planned(&local(&t, "y/A.txt"), "/up/y/A.txt", 2),
    ]);
    t.engine.max_parallel = 0;

    t.engine.apply_plan_ready(batch_id, session, Direction::Upload, plan, &[], false);

    assert!(t.engine.transfers.jobs().all(|job| job.remove_source));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_symlink_selected_directly_is_never_followed_or_moved() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/var/www/index.html", b"site", None).symlink("/srv/www", "/var/www");
    let entry = Entry {
        modified: None,
        name: "www".to_string(),
        path: VfsPath::from("/srv/www"),
        is_dir: true,
        size: 0,
        permissions: None,
        ..Default::default()
    };

    t.engine.move_entries(Location::Session(session), vec![entry], Location::Local, from_os_path(t.dir.path()));
    t.settle().await;

    assert!(remote.exists("/var/www/index.html"));
    assert!(remote.exists("/srv/www"));
    assert!(!local(&t, "www").exists());
    let notices = t.notices();
    assert!(notices.iter().any(|(_, message)| message.contains("1 symlink")), "{notices:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_folder_is_moved_and_its_original_removed() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::create_dir_all(local(&t, "hollow/inner")).unwrap();

    upload(&mut t, session, &["hollow"], "/up").await;

    assert!(remote.exists("/up/hollow/inner"));
    assert!(!local(&t, "hollow").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_the_only_queued_files_still_cleans_up_what_was_emptied() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::create_dir_all(local(&t, "tree/hollow")).unwrap();
    std::fs::write(local(&t, "tree/big.bin"), b"queued").unwrap();
    t.engine.max_parallel = 0;
    let root = local(&t, "tree");
    let batch_id = move_batch(&mut t, session, vec![from_os_path(&root)]);
    let plan = plan_of(vec![planned(&local(&t, "tree/big.bin"), "/up/tree/big.bin", 6)]);
    t.engine.apply_plan_ready(batch_id, session, Direction::Upload, plan, &[], false);
    t.drain();

    t.engine.cancel_row(crate::transfer::rows::RowKind::Batch(batch_id));
    t.settle().await;

    assert!(!local(&t, "tree/hollow").exists());
    assert!(local(&t, "tree/big.bin").exists());
    assert!(changed(&t.drain(), Location::Local));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upload_batch_is_cleaned_up_even_after_its_session_is_gone() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    std::fs::create_dir_all(local(&t, "tree/hollow")).unwrap();
    let root = local(&t, "tree");
    let batch_id = move_batch(&mut t, session, vec![from_os_path(&root)]);
    t.engine.sessions.clear();

    t.engine.finish_move_batch(batch_id);
    t.settle().await;

    assert!(!local(&t, "tree").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn renaming_on_conflict_moves_the_file_under_the_new_name_and_leaves_the_old_one() {
    let mut t = test_engine();
    t.engine.on_conflict = ConflictPolicy::Rename;
    let (session, remote) = t.add_session("srv");
    remote.file("/up/a.txt", b"theirs", None);
    std::fs::write(local(&t, "a.txt"), b"mine").unwrap();

    upload(&mut t, session, &["a.txt"], "/up").await;

    assert_eq!(remote.contents("/up/a.txt").unwrap(), b"theirs");
    assert_eq!(remote.contents("/up/a (1).txt").unwrap(), b"mine");
    assert!(!local(&t, "a.txt").exists());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn retrying_after_the_original_could_not_be_removed_moves_it_once_it_can_be() {
    if is_root() {
        return;
    }
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    std::fs::create_dir(local(&t, "sealed")).unwrap();
    std::fs::write(local(&t, "sealed/a.txt"), b"hello").unwrap();
    std::fs::set_permissions(local(&t, "sealed"), std::os::unix::fs::PermissionsExt::from_mode(0o500)).unwrap();
    let entry = Entry {
        modified: None,
        name: "a.txt".to_string(),
        path: from_os_path(&local(&t, "sealed/a.txt")),
        is_dir: false,
        size: 5,
        permissions: None,
        ..Default::default()
    };
    t.engine.move_entries(Location::Local, vec![entry], Location::Session(session), VfsPath::from("/up"));
    t.settle().await;
    assert!(matches!(t.engine.transfers.jobs().next().unwrap().status, JobStatus::Failed(_)));
    std::fs::set_permissions(local(&t, "sealed"), std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let kind = t.engine.transfers.rows()[0].kind;

    t.engine.retry_row(kind);
    t.settle().await;

    assert!(!local(&t, "sealed/a.txt").exists());
    assert_eq!(remote.contents("/up/a.txt").unwrap(), b"hello");
    assert_eq!(t.engine.transfers.jobs().next().unwrap().status, JobStatus::Completed);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_lost_before_the_original_is_removed_fails_the_job_and_keeps_both() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    let batch_id = move_batch(&mut t, session, Vec::new());
    std::fs::write(local(&t, "a.txt"), b"hello").unwrap();
    let id = t.engine.transfers.enqueue(
        session,
        Direction::Upload,
        from_os_path(&local(&t, "a.txt")),
        "/up/a.txt".to_string(),
        "a.txt".to_string(),
        5,
        Some(batch_id),
    );
    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::InProgress;
    t.engine.sessions.clear();

    t.engine.remove_moved_source(id);

    let job = t.engine.transfers.get(id).unwrap();
    assert!(
        matches!(&job.status, JobStatus::Failed(message) if message.contains("session disconnected")),
        "{:?}",
        job.status
    );
    assert!(local(&t, "a.txt").exists());
}

#[tokio::test]
async fn cancelling_while_the_original_is_about_to_be_removed_keeps_it_and_marks_the_job_cancelled() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/up/a.txt", b"hello", None);
    let batch_id = move_batch(&mut t, session, Vec::new());
    std::fs::write(local(&t, "a.txt"), b"hello").unwrap();
    let id = t.engine.transfers.enqueue(
        session,
        Direction::Upload,
        from_os_path(&local(&t, "a.txt")),
        "/up/a.txt".to_string(),
        "a.txt".to_string(),
        5,
        Some(batch_id),
    );
    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::InProgress;

    t.engine.remove_moved_source(id);
    t.engine.tasks.cancel(crate::tasks::Scope::Transfer(id));
    t.settle().await;

    assert!(local(&t, "a.txt").exists());
    assert_eq!(t.engine.transfers.get(id).unwrap().status, JobStatus::Cancelled);
}

#[test]
fn a_cancelled_scan_forgets_its_move() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    let batch_id = move_batch(&mut t, session, Vec::new());

    t.engine.handle_transfer_event(TransferEvent::PlanCancelled {
        batch_id,
        session_id: session,
        direction: Direction::Upload,
    });

    assert!(t.engine.moves.is_empty());
}

#[test]
fn a_failed_scan_forgets_its_move() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");
    let batch_id = move_batch(&mut t, session, Vec::new());

    t.engine.handle_transfer_event(TransferEvent::PlanFailed { batch_id, message: "no".to_string() });

    assert!(t.engine.moves.is_empty());
}

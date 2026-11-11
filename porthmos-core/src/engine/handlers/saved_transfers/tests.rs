use porthmos_lfs::from_os_path;
use porthmos_vfs::VfsPath;

use crate::{
    Severity,
    engine::{
        Command, Event,
        testing::{TestEngine, test_engine},
    },
    persist::Loaded,
    transfer::{
        Direction, JobStatus,
        rows::{RowKind, RowState},
        saved::{INTERRUPTED_REASON, SavedJob, load, render},
    },
};

fn saved_job(connection: &str, name: &str, direction: Direction) -> SavedJob {
    SavedJob {
        connection: connection.to_string(),
        direction,
        local_path: format!("/local/{name}"),
        remote_path: format!("/remote/{name}"),
        display_name: name.to_string(),
        total_bytes: 100,
        transferred_bytes: 0,
        modified: None,
        remove_source: false,
        batch: None,
        label: None,
        failure: None,
    }
}

fn write_saved(t: &TestEngine, jobs: &[SavedJob]) {
    let path = t.engine.paths.unfinished_transfers_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, render(jobs).unwrap()).unwrap();
}

fn saved_on_disk(t: &TestEngine) -> Vec<SavedJob> {
    match load(&t.engine.paths) {
        Loaded::Ready(jobs) => jobs,
        Loaded::Missing => Vec::new(),
        Loaded::SetAside { warning, .. } => panic!("{warning}"),
    }
}

fn enqueue_upload(t: &mut TestEngine, session: u64, name: &str, batch: Option<u64>) -> u64 {
    t.engine.transfers.enqueue(
        session,
        Direction::Upload,
        VfsPath::new(&format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        100,
        batch,
    )
}

fn job(t: &TestEngine, id: u64) -> crate::transfer::TransferJob {
    t.engine.transfers.get(id).unwrap().clone()
}

fn all_jobs(t: &TestEngine) -> Vec<crate::transfer::TransferJob> {
    t.engine.transfers.jobs().cloned().collect()
}

#[tokio::test]
async fn saving_writes_the_unfinished_jobs_under_their_connection_names() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    enqueue_upload(&mut t, session, "queued.bin", None);
    let running = enqueue_upload(&mut t, session, "running.bin", None);
    let done = enqueue_upload(&mut t, session, "done.bin", None);
    t.engine.transfers.get_mut(running).unwrap().status = JobStatus::InProgress;
    t.engine.transfers.get_mut(running).unwrap().transferred_bytes = 40;
    t.engine.transfers.get_mut(done).unwrap().status = JobStatus::Completed;

    t.engine.save_unfinished_transfers();
    t.engine.writer.flush().await;

    let saved = saved_on_disk(&t);
    assert_eq!(saved.iter().map(|job| job.display_name.as_str()).collect::<Vec<_>>(), ["queued.bin", "running.bin"]);
    assert!(saved.iter().all(|job| job.connection == "srv" && job.direction == Direction::Upload));
    assert_eq!(saved[1].transferred_bytes, 40);
}

#[tokio::test]
async fn saving_with_nothing_unfinished_removes_an_older_file() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "old.bin", Direction::Upload)]);

    t.engine.save_unfinished_transfers();
    t.engine.writer.flush().await;

    assert!(!t.engine.paths.unfinished_transfers_file().exists());
}

#[tokio::test]
async fn saving_with_nothing_unfinished_creates_no_file() {
    let mut t = test_engine();

    t.engine.save_unfinished_transfers();
    t.engine.writer.flush().await;

    assert!(!t.engine.paths.unfinished_transfers_file().exists());
}

#[tokio::test]
async fn restoring_brings_every_saved_job_back_as_an_interrupted_one() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload), saved_job("other", "b.bin", Direction::Download)]);

    t.engine.restore_unfinished_transfers();

    let jobs = all_jobs(&t);
    assert_eq!(jobs.len(), 2);
    for job in &jobs {
        assert_eq!(job.status, JobStatus::Failed(INTERRUPTED_REASON.to_string()));
        assert!(job.resume);
        assert!(job.attempts >= 1);
    }
    assert_eq!(jobs[0].local_path, VfsPath::new("/local/a.bin"));
    assert_eq!(jobs[0].remote_path, "/remote/a.bin");
    assert_eq!(jobs[1].direction, Direction::Download);
    assert_ne!(jobs[0].session_id, jobs[1].session_id);
    assert_eq!(t.engine.session_name(jobs[0].session_id), "srv");
    assert_eq!(t.engine.session_name(jobs[1].session_id), "other");
}

#[tokio::test]
async fn a_job_that_had_failed_for_another_reason_keeps_that_reason() {
    let mut t = test_engine();
    let mut failed = saved_job("srv", "a.bin", Direction::Upload);
    failed.failure = Some("permission denied".to_string());
    write_saved(&t, &[failed]);

    t.engine.restore_unfinished_transfers();

    assert_eq!(all_jobs(&t)[0].status, JobStatus::Failed("permission denied".to_string()));
}

#[tokio::test]
async fn restored_jobs_keep_their_move_flag_times_and_progress() {
    let mut t = test_engine();
    let mut moved = saved_job("srv", "a.bin", Direction::Upload);
    moved.remove_source = true;
    moved.modified = Some(1_700_000_000);
    moved.transferred_bytes = 60;
    write_saved(&t, &[moved]);

    t.engine.restore_unfinished_transfers();

    let restored = &all_jobs(&t)[0];
    assert!(restored.remove_source);
    assert_eq!(restored.modified, Some(1_700_000_000));
    assert_eq!(restored.transferred_bytes, 60);
    assert_eq!(restored.total_bytes, 100);
}

#[tokio::test]
async fn jobs_saved_in_one_batch_come_back_as_one_row() {
    let mut t = test_engine();
    let mut first = saved_job("srv", "a.bin", Direction::Upload);
    let mut second = saved_job("srv", "b.bin", Direction::Upload);
    let mut apart = saved_job("srv", "c.bin", Direction::Upload);
    for job in [&mut first, &mut second] {
        job.batch = Some(4);
        job.label = Some("photos".to_string());
    }
    apart.batch = Some(9);
    apart.label = Some("c.bin".to_string());
    write_saved(&t, &[first, second, apart]);

    t.engine.restore_unfinished_transfers();

    let rows = t.engine.transfers.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].label, "photos");
    assert_eq!(rows[0].files_total, 2);
    assert_eq!(rows[1].label, "c.bin");
    assert!(rows.iter().all(|row| row.state == RowState::Failed));
}

#[tokio::test]
async fn restoring_tells_the_user_what_came_back_and_how_to_continue() {
    let mut t = test_engine();
    write_saved(
        &t,
        &[
            saved_job("srv", "a.bin", Direction::Upload),
            saved_job("srv", "b.bin", Direction::Upload),
            saved_job("other", "c.bin", Direction::Download),
        ],
    );

    t.engine.restore_unfinished_transfers();

    let notices = t.notices();
    assert_eq!(
        notices,
        [(
            Severity::Info,
            "3 transfers from last time are in the transfers list. Connect to other or srv and retry them to continue."
                .to_string()
        )]
    );
}

#[tokio::test]
async fn restoring_one_transfer_uses_the_singular() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload)]);

    t.engine.restore_unfinished_transfers();

    assert_eq!(
        t.notices(),
        [(
            Severity::Info,
            "1 transfer from last time is in the transfers list. Connect to srv and retry it to continue.".to_string()
        )]
    );
}

#[tokio::test]
async fn restoring_nothing_is_silent() {
    let mut t = test_engine();

    t.engine.restore_unfinished_transfers();

    assert!(t.drain().is_empty());
    assert!(all_jobs(&t).is_empty());
}

#[tokio::test]
async fn restoring_publishes_the_rows_to_the_front_ends() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload)]);

    t.engine.restore_unfinished_transfers();

    let snapshot = t
        .drain()
        .into_iter()
        .filter_map(|event| match event {
            Event::TransfersChanged(snapshot) => Some(snapshot),
            _ => None,
        })
        .next_back()
        .expect("a transfers snapshot");
    assert_eq!(snapshot.rows.len(), 1);
    assert_eq!(snapshot.rows[0].state, RowState::Failed);
    assert_eq!(snapshot.rows[0].failure.as_ref().map(|failure| failure.message.as_str()), Some(INTERRUPTED_REASON));
}

#[tokio::test]
async fn restored_rows_do_not_add_history_entries() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload)]);

    t.engine.restore_unfinished_transfers();
    t.engine.process_row_changes();

    assert!(t.engine.history.store.entries().is_empty());
}

#[tokio::test]
async fn an_unreadable_file_is_set_aside_and_reported() {
    let mut t = test_engine();
    let path = t.engine.paths.unfinished_transfers_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "jobs = [[[").unwrap();

    t.engine.restore_unfinished_transfers();

    let notices = t.notices();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].0, Severity::Warning);
    assert!(notices[0].1.starts_with("The unfinished transfers file was unreadable"), "{}", notices[0].1);
    assert!(all_jobs(&t).is_empty());
}

#[tokio::test]
async fn retrying_after_connecting_to_the_same_connection_continues_where_it_stopped() {
    let mut t = test_engine();
    let mut interrupted = saved_job("srv", "a.bin", Direction::Upload);
    interrupted.local_path = from_os_path(&t.dir.path().join("a.bin")).to_string_lossy().into_owned();
    interrupted.transferred_bytes = 50;
    std::fs::write(t.dir.path().join("a.bin"), vec![1u8; 100]).unwrap();
    write_saved(&t, &[interrupted]);
    t.engine.restore_unfinished_transfers();
    let id = all_jobs(&t)[0].id;
    let (session, remote) = t.add_session("srv");
    remote.dir("/remote");
    t.drain();

    t.engine.handle_command(Command::RetryRow { kind: RowKind::Single(id) });

    let retried = job(&t, id);
    assert_eq!(retried.session_id, session);
    assert!(retried.resume);
    assert!(matches!(retried.status, JobStatus::Queued | JobStatus::InProgress), "{:?}", retried.status);
    t.settle().await;
    assert_eq!(remote.contents("/remote/a.bin").unwrap().len(), 100);
}

#[tokio::test]
async fn retrying_without_that_connection_asks_to_connect_first() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload)]);
    t.engine.restore_unfinished_transfers();
    let id = all_jobs(&t)[0].id;
    t.drain();

    t.engine.handle_command(Command::RetryRow { kind: RowKind::Single(id) });

    assert_eq!(t.notices(), [(Severity::Warning, "Can't retry: reconnect to srv first".to_string())]);
    assert!(matches!(job(&t, id).status, JobStatus::Failed(_)));
}

#[tokio::test]
async fn clearing_a_restored_download_removes_its_leftover_part_file() {
    let mut t = test_engine();
    let target = t.dir.path().join("big.iso");
    let part = t.dir.path().join("big.iso.part");
    std::fs::write(&part, vec![7u8; 30]).unwrap();
    let mut interrupted = saved_job("srv", "big.iso", Direction::Download);
    interrupted.local_path = from_os_path(&target).to_string_lossy().into_owned();
    write_saved(&t, &[interrupted]);
    t.engine.restore_unfinished_transfers();
    t.drain();

    t.engine.handle_command(Command::ClearFinished);

    assert!(!part.exists());
    assert!(all_jobs(&t).is_empty());
}

#[tokio::test]
async fn what_was_saved_comes_back_in_a_second_engine() {
    let mut first = test_engine();
    let (session, _) = first.add_session("srv");
    let batch = first.engine.transfers.start_batch("photos".to_string());
    enqueue_upload(&mut first, session, "a.bin", Some(batch));
    enqueue_upload(&mut first, session, "b.bin", Some(batch));
    first.engine.save_unfinished_transfers();
    first.engine.writer.flush().await;
    let saved = std::fs::read(first.engine.paths.unfinished_transfers_file()).unwrap();

    let mut second = test_engine();
    let path = second.engine.paths.unfinished_transfers_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, saved).unwrap();
    second.engine.restore_unfinished_transfers();

    let rows = second.engine.transfers.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "photos");
    assert_eq!(rows[0].files_total, 2);
    assert_eq!(second.engine.session_name(all_jobs(&second)[0].session_id), "srv");
}

#[tokio::test]
async fn shutting_down_saves_the_queue_before_it_is_cancelled() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    let running = enqueue_upload(&mut t, session, "running.bin", None);
    enqueue_upload(&mut t, session, "waiting.bin", None);
    t.engine.transfers.get_mut(running).unwrap().status = JobStatus::InProgress;

    t.engine.shutdown_work(&mut t.internal).await;

    let saved = saved_on_disk(&t);
    assert_eq!(saved.iter().map(|job| job.display_name.as_str()).collect::<Vec<_>>(), ["running.bin", "waiting.bin"]);
    assert!(saved.iter().all(|job| job.connection == "srv"));
}

#[tokio::test]
async fn a_restored_job_nobody_touched_is_saved_again_with_its_reason() {
    let mut t = test_engine();
    let mut failed = saved_job("srv", "a.bin", Direction::Upload);
    failed.failure = Some("permission denied".to_string());
    write_saved(&t, &[saved_job("srv", "b.bin", Direction::Upload), failed]);
    t.engine.restore_unfinished_transfers();

    t.engine.save_unfinished_transfers();

    let saved = saved_on_disk(&t);
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0].failure.as_deref(), Some(INTERRUPTED_REASON));
    assert_eq!(saved[1].failure.as_deref(), Some("permission denied"));
    assert!(saved.iter().all(|job| job.connection == "srv"));
}

#[tokio::test]
async fn a_file_that_could_not_be_set_aside_is_never_overwritten() {
    let mut t = test_engine();
    t.engine.unfinished_protected = true;
    let (session, _) = t.add_session("srv");
    enqueue_upload(&mut t, session, "a.bin", None);
    write_saved(&t, &[saved_job("srv", "kept.bin", Direction::Upload)]);

    t.engine.save_unfinished_transfers();

    assert_eq!(saved_on_disk(&t)[0].display_name, "kept.bin");
}

#[tokio::test]
async fn the_saved_file_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    enqueue_upload(&mut t, session, "a.bin", None);

    t.engine.save_unfinished_transfers();

    let mode = std::fs::metadata(t.engine.paths.unfinished_transfers_file()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[tokio::test]
async fn the_published_rows_name_the_connections_that_are_not_open() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload), saved_job("other", "b.bin", Direction::Upload)]);

    t.engine.restore_unfinished_transfers();

    let snapshot = t
        .drain()
        .into_iter()
        .filter_map(|event| match event {
            Event::TransfersChanged(snapshot) => Some(snapshot),
            _ => None,
        })
        .next_back()
        .expect("a transfers snapshot");
    let names: Vec<&str> = snapshot.connections.values().map(String::as_str).collect();
    assert_eq!(names, ["srv", "other"]);
    for row in &snapshot.rows {
        assert!(snapshot.connections.contains_key(&row.session_id.unwrap()));
    }
}

#[tokio::test]
async fn an_open_connection_is_not_repeated_in_the_published_names() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    enqueue_upload(&mut t, session, "a.bin", None);

    let snapshot = t.engine.snapshot();

    assert!(snapshot.connections.is_empty(), "{:?}", snapshot.connections);
}

#[tokio::test]
async fn a_closed_connection_keeps_its_name_in_the_published_rows() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    t.engine.history.remember_session(session, "srv".to_string());
    enqueue_upload(&mut t, session, "a.bin", None);
    t.engine.sessions.remove(&session);

    let snapshot = t.engine.snapshot();

    assert_eq!(snapshot.connections.get(&session).map(String::as_str), Some("srv"));
}

#[tokio::test]
async fn clearing_a_restored_upload_removes_its_remote_part_file_through_the_open_connection() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "big.iso", Direction::Upload)]);
    t.engine.restore_unfinished_transfers();
    let (_, remote) = t.add_session("srv");
    remote.dir("/remote").file("/remote/big.iso.part", b"half", None);
    t.drain();

    t.engine.handle_command(Command::ClearFinished);
    t.settle().await;

    assert!(!remote.exists("/remote/big.iso.part"));
    assert!(all_jobs(&t).is_empty());
}

#[tokio::test]
async fn clearing_a_restored_upload_leaves_a_remote_part_file_alone_when_that_connection_is_not_open() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "big.iso", Direction::Upload)]);
    t.engine.restore_unfinished_transfers();
    let (_, other) = t.add_session("other");
    other.dir("/remote").file("/remote/big.iso.part", b"half", None);
    t.drain();

    t.engine.handle_command(Command::ClearFinished);
    t.settle().await;

    assert!(other.exists("/remote/big.iso.part"));
}

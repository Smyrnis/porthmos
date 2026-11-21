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
        saved::{SavedJob, interrupted_reason, load, render},
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
    let path = t.engine.host.paths.unfinished_transfers_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, render(jobs).unwrap()).unwrap();
}

fn saved_on_disk(t: &TestEngine) -> Vec<SavedJob> {
    match load(&t.engine.host.paths) {
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

    assert!(!t.engine.host.paths.unfinished_transfers_file().exists());
}

#[tokio::test]
async fn saving_with_nothing_unfinished_creates_no_file() {
    let mut t = test_engine();

    t.engine.save_unfinished_transfers();
    t.engine.writer.flush().await;

    assert!(!t.engine.host.paths.unfinished_transfers_file().exists());
}

#[tokio::test]
async fn restoring_brings_every_saved_job_back_as_an_interrupted_one() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload), saved_job("other", "b.bin", Direction::Download)]);

    t.engine.restore_unfinished_transfers();

    let jobs = all_jobs(&t);
    assert_eq!(jobs.len(), 2);
    for job in &jobs {
        assert_eq!(job.status, JobStatus::Failed(interrupted_reason()));
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

    assert_eq!(all_jobs(&t)[0].status, JobStatus::Failed("permission denied".into()));
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
    assert_eq!(snapshot.rows[0].failure.as_ref().map(|failure| failure.message.clone()), Some(interrupted_reason()));
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
    let path = t.engine.host.paths.unfinished_transfers_file();
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
    let saved = std::fs::read(first.engine.host.paths.unfinished_transfers_file()).unwrap();

    let mut second = test_engine();
    let path = second.engine.host.paths.unfinished_transfers_file();
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
    assert_eq!(saved[0].failure.as_deref(), Some(interrupted_reason().to_string().as_str()));
    assert_eq!(saved[1].failure.as_deref(), Some("permission denied"));
    assert!(saved.iter().all(|job| job.connection == "srv"));
}

#[tokio::test]
async fn a_file_that_could_not_be_set_aside_is_never_overwritten() {
    let mut t = test_engine();
    t.engine.unfinished_saves.protected = true;
    let (session, _) = t.add_session("srv");
    enqueue_upload(&mut t, session, "a.bin", None);
    write_saved(&t, &[saved_job("srv", "kept.bin", Direction::Upload)]);

    t.engine.save_unfinished_transfers();

    assert_eq!(saved_on_disk(&t)[0].display_name, "kept.bin");
}

#[cfg(unix)]
#[tokio::test]
async fn the_saved_file_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    enqueue_upload(&mut t, session, "a.bin", None);

    t.engine.save_unfinished_transfers();

    let mode = std::fs::metadata(t.engine.host.paths.unfinished_transfers_file()).unwrap().permissions().mode();
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
    t.engine.book.sessions.remove(&session);

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

fn ready_to_save(t: &mut TestEngine) {
    t.engine.restore_unfinished_transfers();
    t.drain();
}

#[tokio::test]
async fn a_new_unfinished_job_reaches_the_disk_when_it_is_published() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    ready_to_save(&mut t);

    enqueue_upload(&mut t, session, "queued.bin", None);
    t.engine.publish_transfers();
    t.engine.writer.flush().await;

    let saved = saved_on_disk(&t);
    assert_eq!(saved.iter().map(|job| job.display_name.as_str()).collect::<Vec<_>>(), ["queued.bin"]);
}

#[tokio::test]
async fn the_file_goes_away_when_the_last_unfinished_job_finishes() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    ready_to_save(&mut t);
    let id = enqueue_upload(&mut t, session, "queued.bin", None);
    t.engine.publish_transfers();
    assert!(t.engine.host.paths.unfinished_transfers_file().exists());

    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::Completed;
    t.engine.publish_transfers();

    assert!(!t.engine.host.paths.unfinished_transfers_file().exists());
}

#[tokio::test]
async fn a_job_that_fails_is_saved_with_its_failure() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    ready_to_save(&mut t);
    let id = enqueue_upload(&mut t, session, "queued.bin", None);
    t.engine.publish_transfers();

    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::Failed("Permission denied".into());
    t.engine.publish_transfers();

    assert_eq!(saved_on_disk(&t)[0].failure.as_deref(), Some("Permission denied"));
}

#[tokio::test]
async fn nothing_is_written_before_the_last_runs_list_has_been_restored() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    write_saved(&t, &[saved_job("srv", "old.bin", Direction::Upload)]);

    enqueue_upload(&mut t, session, "new.bin", None);
    t.engine.publish_transfers();

    let saved = saved_on_disk(&t);
    assert_eq!(saved.iter().map(|job| job.display_name.as_str()).collect::<Vec<_>>(), ["old.bin"]);
}

#[tokio::test]
async fn a_protected_file_is_never_overwritten_by_a_publish() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    ready_to_save(&mut t);
    t.engine.unfinished_saves.protected = true;

    enqueue_upload(&mut t, session, "queued.bin", None);
    t.engine.publish_transfers();

    assert!(!t.engine.host.paths.unfinished_transfers_file().exists());
}

#[tokio::test]
async fn progress_alone_does_not_rewrite_the_file_on_every_publish() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    ready_to_save(&mut t);
    let id = enqueue_upload(&mut t, session, "big.bin", None);
    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::InProgress;
    t.engine.publish_transfers();

    t.engine.transfers.get_mut(id).unwrap().transferred_bytes = 40;
    t.engine.publish_transfers();

    assert_eq!(saved_on_disk(&t)[0].transferred_bytes, 0);
}

#[tokio::test(start_paused = true)]
async fn progress_is_saved_after_a_while_so_a_crash_resumes_near_where_it_stopped() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    ready_to_save(&mut t);
    let id = enqueue_upload(&mut t, session, "big.bin", None);
    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::InProgress;
    t.engine.publish_transfers();

    t.engine.transfers.get_mut(id).unwrap().transferred_bytes = 40;
    tokio::time::sleep(super::UNFINISHED_PROGRESS_SAVE + std::time::Duration::from_secs(1)).await;
    t.engine.publish_transfers();

    assert_eq!(saved_on_disk(&t)[0].transferred_bytes, 40);
}

#[tokio::test]
async fn restoring_the_list_makes_saving_possible_even_when_nothing_was_saved() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");

    t.engine.restore_unfinished_transfers();
    enqueue_upload(&mut t, session, "queued.bin", None);
    t.engine.publish_transfers();

    assert_eq!(saved_on_disk(&t).len(), 1);
}

fn move_batch(session_id: u64, direction: Direction, roots: &[&str]) -> crate::engine::handlers::MoveBatch {
    crate::engine::handlers::MoveBatch {
        session_id,
        direction,
        roots: roots.iter().map(|root| VfsPath::new(root)).collect(),
    }
}

fn saved_moves_on_disk(t: &TestEngine) -> Vec<crate::transfer::saved::SavedMove> {
    crate::transfer::saved::load_moves(&t.engine.host.paths)
}

#[tokio::test]
async fn the_folders_a_move_should_clean_up_are_saved_with_the_unfinished_jobs() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    let batch = t.engine.transfers.start_batch("photos".to_string());
    enqueue_upload(&mut t, session, "a.bin", Some(batch));
    t.engine.work.moves.insert(batch, move_batch(session, Direction::Upload, &["/local/photos"]));

    t.engine.save_unfinished_transfers();

    let moves = saved_moves_on_disk(&t);
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].connection, "srv");
    assert_eq!(moves[0].direction, Direction::Upload);
    assert_eq!(moves[0].roots, ["/local/photos"]);
    assert_eq!(moves[0].batch, saved_on_disk(&t)[0].batch.unwrap());
}

#[tokio::test]
async fn a_move_with_nothing_left_to_do_is_not_saved() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    let finished = t.engine.transfers.start_batch("done".to_string());
    let done = enqueue_upload(&mut t, session, "d.bin", Some(finished));
    t.engine.transfers.get_mut(done).unwrap().status = JobStatus::Completed;
    t.engine.work.moves.insert(finished, move_batch(session, Direction::Upload, &["/local/done"]));
    let open = t.engine.transfers.start_batch("open".to_string());
    enqueue_upload(&mut t, session, "o.bin", Some(open));

    t.engine.save_unfinished_transfers();

    assert!(saved_moves_on_disk(&t).is_empty());
}

#[tokio::test]
async fn a_batch_without_folders_to_clean_is_not_saved_as_a_move() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    let batch = t.engine.transfers.start_batch("files".to_string());
    enqueue_upload(&mut t, session, "a.bin", Some(batch));
    t.engine.work.moves.insert(batch, move_batch(session, Direction::Upload, &[]));

    t.engine.save_unfinished_transfers();

    assert!(saved_moves_on_disk(&t).is_empty());
}

#[tokio::test]
async fn restoring_a_move_brings_back_its_folder_cleanup_under_the_new_batch_and_session() {
    let mut first = test_engine();
    let (session, _) = first.add_session("srv");
    let batch = first.engine.transfers.start_batch("photos".to_string());
    enqueue_upload(&mut first, session, "a.bin", Some(batch));
    first.engine.work.moves.insert(batch, move_batch(session, Direction::Upload, &["/local/photos", "/local/other"]));
    first.engine.save_unfinished_transfers();
    let saved = std::fs::read(first.engine.host.paths.unfinished_transfers_file()).unwrap();

    let mut second = test_engine();
    let path = second.engine.host.paths.unfinished_transfers_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, saved).unwrap();
    second.engine.restore_unfinished_transfers();

    let restored = all_jobs(&second);
    let new_batch = restored[0].batch_id.expect("the job is still in a batch");
    let moved = second.engine.work.moves.get(&new_batch).expect("the move bookkeeping is back");
    assert_eq!(moved.session_id, restored[0].session_id);
    assert_eq!(moved.direction, Direction::Upload);
    assert_eq!(moved.roots, [VfsPath::new("/local/photos"), VfsPath::new("/local/other")]);
}

#[tokio::test]
async fn a_saved_move_whose_jobs_are_gone_is_ignored_on_restore() {
    let mut t = test_engine();
    write_saved_with_moves(
        &t,
        &[saved_job("srv", "a.bin", Direction::Upload)],
        &[crate::transfer::saved::SavedMove {
            batch: 77,
            connection: "srv".to_string(),
            direction: Direction::Upload,
            roots: vec!["/local/x".to_string()],
        }],
    );

    t.engine.restore_unfinished_transfers();

    assert!(t.engine.work.moves.is_empty());
}

fn write_saved_with_moves(t: &TestEngine, jobs: &[SavedJob], moves: &[crate::transfer::saved::SavedMove]) {
    let path = t.engine.host.paths.unfinished_transfers_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, crate::transfer::saved::render_with_moves(jobs, moves).unwrap()).unwrap();
}

#[tokio::test]
async fn a_file_saved_before_moves_were_recorded_still_loads() {
    let mut t = test_engine();
    write_saved(&t, &[saved_job("srv", "a.bin", Direction::Upload)]);

    t.engine.restore_unfinished_transfers();

    assert_eq!(all_jobs(&t).len(), 1);
    assert!(t.engine.work.moves.is_empty());
}

#[tokio::test]
async fn a_restored_download_move_removes_its_emptied_folders_once_the_jobs_are_done() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/srv/photos").dir("/srv/photos/sub");
    let batch = t.engine.transfers.start_batch("photos".to_string());
    let id = t.engine.transfers.enqueue(
        session,
        Direction::Download,
        VfsPath::new("/local/a.bin"),
        "/srv/photos/a.bin".to_string(),
        "a.bin".to_string(),
        100,
        Some(batch),
    );
    t.engine.work.moves.insert(batch, move_batch(session, Direction::Download, &["/srv/photos"]));
    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::Completed;

    t.engine.finish_move_batch(batch);
    t.settle().await;

    assert!(!remote.exists("/srv/photos"));
}

#[tokio::test]
async fn a_file_from_the_current_release_restores_its_failure() {
    let mut t = test_engine();
    let mut with_cause = saved_job("srv", "a.bin", Direction::Upload);
    with_cause.failure = Some("Could not copy a.bin:\ndisk full".to_string());
    let mut plain = saved_job("srv", "b.bin", Direction::Upload);
    plain.failure = Some("Session disconnected".to_string());
    write_saved(&t, &[with_cause, plain, saved_job("srv", "c.bin", Direction::Upload)]);

    t.engine.restore_unfinished_transfers();

    let jobs = all_jobs(&t);
    let failures: Vec<&crate::Message> = jobs
        .iter()
        .map(|job| match &job.status {
            JobStatus::Failed(message) => message,
            other => panic!("expected a failed job, got {other:?}"),
        })
        .collect();
    assert_eq!((failures[0].summary(), failures[0].cause()), ("Could not copy a.bin", Some("disk full")));
    assert_eq!((failures[1].summary(), failures[1].cause()), ("Session disconnected", None));
    assert_eq!(*failures[2], interrupted_reason());
}

#[tokio::test]
async fn a_failed_job_comes_back_equal_after_save_and_restore() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");
    ready_to_save(&mut t);
    let id = enqueue_upload(&mut t, session, "a.bin", None);
    let failure = crate::error::failure_report("Could not copy a.bin", &"disk full");
    t.engine.transfers.get_mut(id).unwrap().status = JobStatus::Failed(failure.clone());
    t.engine.publish_transfers();
    let saved = saved_on_disk(&t);

    let mut restarted = test_engine();
    write_saved(&restarted, &saved);
    restarted.engine.restore_unfinished_transfers();

    assert_eq!(all_jobs(&restarted)[0].status, JobStatus::Failed(failure));
}

#[tokio::test]
async fn a_failure_text_with_line_breaks_restores_as_the_same_text() {
    let mut t = test_engine();
    let mut multi_line = saved_job("srv", "a.bin", Direction::Upload);
    multi_line.failure = Some("copied, but the original couldn't be removed: 550-a\n550 b".to_string());
    let mut odd_summary = saved_job("srv", "b.bin", Direction::Upload);
    odd_summary.failure = Some("Transfer failed: b\nname:\nerr".to_string());
    write_saved(&t, &[multi_line, odd_summary]);

    t.engine.restore_unfinished_transfers();

    let texts: Vec<String> = all_jobs(&t)
        .iter()
        .map(|job| match &job.status {
            JobStatus::Failed(message) => message.to_string(),
            other => panic!("expected a failed job, got {other:?}"),
        })
        .collect();
    assert_eq!(texts[0], "copied, but the original couldn't be removed: 550-a\n550 b");
    assert_eq!(texts[1], "Transfer failed: b\nname:\nerr");
}

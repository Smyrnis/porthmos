use std::time::Duration;

use porthmos_lfs::from_os_path;
use porthmos_vfs::{FileSystem, VfsPath, testing::FakeFs};

use crate::{
    Severity,
    engine::{
        Command, Event, Location, TransferEvent,
        testing::{TestEngine, test_engine},
    },
    transfer::{
        Direction, JobStatus, TransferOutcome,
        rows::{RowKind, RowState},
    },
};

fn batch_of(t: &mut TestEngine, session: u64, names: &[&str], size: usize) -> (u64, Vec<u64>) {
    let batch = t.engine.transfers.start_batch("batch".to_string());
    let ids = names
        .iter()
        .map(|name| {
            std::fs::write(t.dir.path().join(name), vec![5u8; size]).unwrap();
            t.engine.transfers.enqueue(
                session,
                Direction::Upload,
                from_os_path(&t.dir.path().join(name)),
                format!("/up/{name}"),
                name.to_string(),
                size as u64,
                Some(batch),
            )
        })
        .collect();
    (batch, ids)
}

fn state_of(t: &TestEngine, kind: RowKind) -> RowState {
    t.engine.transfers.row(kind).unwrap().state
}

fn job(t: &TestEngine, id: u64) -> crate::transfer::TransferJob {
    t.engine.transfers.get(id).unwrap().clone()
}

fn row_states(events: &[Event]) -> Vec<RowState> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::TransfersChanged(snapshot) => Some(snapshot.rows.iter().map(|row| row.state).collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn engine_with_idle_queue(names: &[&str]) -> (TestEngine, FakeFs, u64, Vec<u64>) {
    let mut t = test_engine();
    t.engine.max_parallel = 0;
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    let (batch, ids) = batch_of(&mut t, session, names, 4000);
    (t, remote, batch, ids)
}

fn engine_with_slow_queue(names: &[&str]) -> (TestEngine, FakeFs, u64, Vec<u64>) {
    let mut t = test_engine();
    t.engine.max_parallel = 0;
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    let (batch, ids) = batch_of(&mut t, session, names, 40_000);
    t.engine.speed_limit.set(Some(4_000));
    (t, remote, batch, ids)
}

async fn wait_for_partial(remote: &FakeFs, path: &str) {
    for _ in 0..200 {
        if remote.exists(path) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("{path} never appeared");
}

#[test]
fn pausing_a_queued_row_pauses_every_job_and_publishes_the_paused_row() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin", "b.bin"]);
    t.drain();

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    assert!(ids.iter().all(|id| job(&t, *id).paused));
    assert_eq!(state_of(&t, RowKind::Batch(batch)), RowState::Paused);
    assert!(row_states(&t.drain()).contains(&RowState::Paused));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paused_row_does_not_start_when_slots_are_free() {
    let (mut t, remote, batch, ids) = engine_with_idle_queue(&["a.bin", "b.bin"]);
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });
    t.engine.max_parallel = 4;

    t.engine.fill_transfer_slots();
    t.settle().await;

    assert!(ids.iter().all(|id| job(&t, *id).status == JobStatus::Queued));
    assert!(!remote.exists("/up/a.bin"));
}

#[tokio::test(flavor = "multi_thread")]
async fn resuming_a_row_lets_its_jobs_run_to_completion() {
    let (mut t, remote, batch, ids) = engine_with_idle_queue(&["a.bin", "b.bin"]);
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });
    t.engine.max_parallel = 4;

    t.engine.handle_command(Command::ResumeRow { kind: RowKind::Batch(batch) });
    t.settle().await;

    assert!(ids.iter().all(|id| job(&t, *id).status == JobStatus::Completed));
    assert_eq!(remote.contents("/up/a.bin").unwrap().len(), 4000);
    assert_eq!(remote.contents("/up/b.bin").unwrap().len(), 4000);
    assert_eq!(state_of(&t, RowKind::Batch(batch)), RowState::Done);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paused_row_lets_the_next_row_use_the_slot() {
    let mut t = test_engine();
    t.engine.max_parallel = 1;
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    t.engine.max_parallel = 0;
    let (first, _) = batch_of(&mut t, session, &["a.bin"], 4000);
    let (_second, second_ids) = batch_of(&mut t, session, &["b.bin"], 4000);
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(first) });
    t.engine.max_parallel = 1;

    t.engine.fill_transfer_slots();
    t.settle().await;

    assert_eq!(job(&t, second_ids[0]).status, JobStatus::Completed);
    assert!(!remote.exists("/up/a.bin"));
}

#[test]
fn pausing_a_row_that_does_not_exist_or_is_only_a_scan_changes_nothing() {
    let mut t = test_engine();

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(99) });
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Scan(3) });
    t.engine.handle_command(Command::ResumeRow { kind: RowKind::Single(7) });

    assert!(t.drain().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn pausing_a_row_that_already_finished_changes_nothing() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    t.engine.max_parallel = 4;
    t.engine.fill_transfer_slots();
    t.settle().await;
    assert_eq!(job(&t, ids[0]).status, JobStatus::Completed);

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    assert!(!job(&t, ids[0]).paused);
    assert_eq!(state_of(&t, RowKind::Batch(batch)), RowState::Done);
}

#[tokio::test(flavor = "multi_thread")]
async fn pausing_a_running_transfer_stops_it_keeps_the_partial_file_and_queues_it_paused() {
    let (mut t, remote, batch, ids) = engine_with_slow_queue(&["a.bin"]);
    t.engine.max_parallel = 4;
    t.engine.fill_transfer_slots();
    wait_for_partial(&remote, "/up/a.bin.part").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(job(&t, ids[0]).status, JobStatus::InProgress);

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });
    t.settle().await;

    let paused = job(&t, ids[0]);
    assert_eq!(paused.status, JobStatus::Queued);
    assert!(paused.paused);
    assert_eq!(paused.attempts, 0);
    assert_eq!(state_of(&t, RowKind::Batch(batch)), RowState::Paused);
    assert!(remote.exists("/up/a.bin.part"));
    assert!(!remote.exists("/up/a.bin"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paused_transfer_resumes_and_finishes_with_every_byte() {
    let (mut t, remote, batch, ids) = engine_with_slow_queue(&["a.bin"]);
    t.engine.max_parallel = 4;
    t.engine.fill_transfer_slots();
    wait_for_partial(&remote, "/up/a.bin.part").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });
    t.settle().await;

    t.engine.handle_command(Command::SetSpeedLimit { bytes_per_second: None });
    t.engine.handle_command(Command::ResumeRow { kind: RowKind::Batch(batch) });
    t.settle().await;

    assert_eq!(job(&t, ids[0]).status, JobStatus::Completed);
    assert_eq!(remote.contents("/up/a.bin").unwrap(), vec![5u8; 40_000]);
    assert!(!remote.exists("/up/a.bin.part"));
}

#[tokio::test(flavor = "multi_thread")]
async fn pausing_and_resuming_before_the_stopped_transfer_reports_back_keeps_it_going() {
    let (mut t, remote, batch, ids) = engine_with_slow_queue(&["a.bin"]);
    t.engine.max_parallel = 4;
    t.engine.fill_transfer_slots();
    wait_for_partial(&remote, "/up/a.bin.part").await;

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });
    t.engine.handle_command(Command::SetSpeedLimit { bytes_per_second: None });
    t.engine.handle_command(Command::ResumeRow { kind: RowKind::Batch(batch) });
    t.settle().await;

    assert_eq!(job(&t, ids[0]).status, JobStatus::Completed);
    assert_eq!(remote.contents("/up/a.bin").unwrap().len(), 40_000);
}

#[test]
fn a_cancelled_report_for_a_paused_running_job_queues_it_again_still_paused() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    {
        let mut running = t.engine.transfers.get_mut(ids[0]).unwrap();
        running.status = JobStatus::InProgress;
        running.transferred_bytes = 100;
    }
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_transfer_event(TransferEvent::Finished { id: ids[0], outcome: TransferOutcome::Cancelled });

    let paused = job(&t, ids[0]);
    assert_eq!(paused.status, JobStatus::Queued);
    assert!(paused.paused && paused.resume);
}

#[test]
fn a_cancelled_failure_for_a_paused_running_job_queues_it_again_still_paused() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    t.engine.transfers.get_mut(ids[0]).unwrap().status = JobStatus::InProgress;
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_transfer_event(TransferEvent::Failed { id: ids[0], message: "aborted".to_string() });

    let paused = job(&t, ids[0]);
    assert_eq!(paused.status, JobStatus::Queued);
    assert!(paused.paused);
    assert!(t.notices().iter().all(|(severity, _)| *severity != Severity::Error));
}

#[test]
fn a_job_that_completes_while_being_paused_stays_completed() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    t.engine.transfers.get_mut(ids[0]).unwrap().status = JobStatus::InProgress;
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_transfer_event(TransferEvent::Finished { id: ids[0], outcome: TransferOutcome::Completed });

    let done = job(&t, ids[0]);
    assert_eq!(done.status, JobStatus::Completed);
    assert!(!done.paused);
    assert_eq!(state_of(&t, RowKind::Batch(batch)), RowState::Done);
}

#[test]
fn cancelling_a_paused_row_cancels_its_jobs_and_a_retry_runs_them() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin", "b.bin"]);
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_command(Command::CancelRow { kind: RowKind::Batch(batch) });

    assert!(ids.iter().all(|id| job(&t, *id).status == JobStatus::Cancelled));
    assert_eq!(state_of(&t, RowKind::Batch(batch)), RowState::Cancelled);

    t.engine.handle_command(Command::RetryRow { kind: RowKind::Batch(batch) });

    assert!(ids.iter().all(|id| !job(&t, *id).paused && job(&t, *id).status == JobStatus::Queued));
}

#[tokio::test]
async fn a_paused_move_that_lost_its_stop_report_does_not_remove_the_original() {
    let (mut t, remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    remote.file("/up/a.bin", &vec![5u8; 4000], None);
    {
        let mut moved = t.engine.transfers.get_mut(ids[0]).unwrap();
        moved.remove_source = true;
        moved.status = JobStatus::InProgress;
    }
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_transfer_event(TransferEvent::SourceRemovalCancelled { id: ids[0] });

    assert!(t.dir.path().join("a.bin").exists());
    let paused = job(&t, ids[0]);
    assert_eq!(paused.status, JobStatus::Queued);
    assert!(paused.paused);
    assert_eq!(remote.stat(&VfsPath::new("/up/a.bin")).await.unwrap().size, 4000);
}

#[test]
fn pausing_does_not_touch_locations_or_publish_a_notice() {
    let (mut t, _remote, batch, _ids) = engine_with_idle_queue(&["a.bin"]);
    t.drain();

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    let events = t.drain();
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, Event::Notice { .. } | Event::LocationChanged { location: Location::Local })),
        "{events:?}"
    );
}

fn mark_running(t: &mut TestEngine, id: u64, bytes: u64) {
    let mut running = t.engine.transfers.get_mut(id).unwrap();
    running.status = JobStatus::InProgress;
    running.transferred_bytes = bytes;
}

fn local_changed(events: &[Event], location: Location) -> bool {
    events.iter().any(|event| matches!(event, Event::LocationChanged { location: seen } if *seen == location))
}

#[test]
fn a_stopped_job_with_no_progress_keeps_the_planners_choice_about_resuming() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin", "b.bin"]);
    mark_running(&mut t, ids[0], 0);
    mark_running(&mut t, ids[1], 700);
    t.engine.transfers.get_mut(ids[1]).unwrap().resume = false;
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    for id in &ids {
        t.engine.handle_transfer_event(TransferEvent::Finished { id: *id, outcome: TransferOutcome::Cancelled });
    }

    assert!(!job(&t, ids[0]).resume);
    assert!(job(&t, ids[1]).resume);
}

#[test]
fn a_stop_takes_back_only_the_attempt_it_used() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    mark_running(&mut t, ids[0], 100);
    t.engine.transfers.get_mut(ids[0]).unwrap().attempts = 2;
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_transfer_event(TransferEvent::Finished { id: ids[0], outcome: TransferOutcome::Cancelled });

    assert_eq!(job(&t, ids[0]).attempts, 1);
}

#[test]
fn a_paused_move_forgets_its_progress_because_it_will_start_over() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    mark_running(&mut t, ids[0], 2500);
    t.engine.transfers.get_mut(ids[0]).unwrap().remove_source = true;
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_transfer_event(TransferEvent::Finished { id: ids[0], outcome: TransferOutcome::Cancelled });

    let moved = job(&t, ids[0]);
    assert_eq!(moved.transferred_bytes, 0);
    assert_eq!(moved.status, JobStatus::Queued);
}

#[test]
fn a_paused_job_holding_a_partial_file_keeps_other_rows_off_its_destination() {
    let mut t = test_engine();
    t.engine.max_parallel = 0;
    let (session, _remote) = t.add_session("srv");
    let enqueue = |t: &mut TestEngine, name: &str| {
        let batch = t.engine.transfers.start_batch(name.to_string());
        t.engine.transfers.enqueue(
            session,
            Direction::Upload,
            from_os_path(&t.dir.path().join(name)),
            "/up/x.txt".to_string(),
            name.to_string(),
            10,
            Some(batch),
        );
        RowKind::Batch(batch)
    };
    let first = enqueue(&mut t, "dir1");
    let second = enqueue(&mut t, "dir2");
    let first_job = t.engine.transfers.jobs_of(first)[0].id;
    {
        let mut job = t.engine.transfers.get_mut(first_job).unwrap();
        job.resume = true;
        job.transferred_bytes = 5;
    }
    t.engine.handle_command(Command::PauseRow { kind: first });
    t.engine.max_parallel = 4;

    t.engine.fill_transfer_slots();

    assert!(t.engine.transfers.jobs_of(second).iter().all(|job| job.status == JobStatus::Queued));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_row_refreshes_the_panel_even_while_another_row_stays_paused() {
    let mut t = test_engine();
    t.engine.max_parallel = 0;
    let (session, remote) = t.add_session("srv");
    remote.dir("/up");
    let (first, _) = batch_of(&mut t, session, &["a.bin"], 100);
    let (second, _) = batch_of(&mut t, session, &["b.bin"], 100);
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(second) });
    t.engine.max_parallel = 4;
    t.drain();

    t.engine.fill_transfer_slots();
    t.settle().await;

    assert_eq!(state_of(&t, RowKind::Batch(first)), RowState::Done);
    assert!(local_changed(&t.drain(), Location::Session(session)));
}

#[test]
fn pausing_the_last_waiting_row_refreshes_the_panel_that_was_waiting_for_it() {
    let mut t = test_engine();
    t.engine.max_parallel = 0;
    let (session, _remote) = t.add_session("srv");
    let (first, first_ids) = batch_of(&mut t, session, &["a.bin"], 100);
    let (second, _) = batch_of(&mut t, session, &["b.bin"], 100);
    mark_running(&mut t, first_ids[0], 100);
    t.engine.handle_transfer_event(TransferEvent::Finished { id: first_ids[0], outcome: TransferOutcome::Completed });
    assert_eq!(state_of(&t, RowKind::Batch(first)), RowState::Done);
    assert!(!local_changed(&t.drain(), Location::Session(session)));

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(second) });

    assert!(local_changed(&t.drain(), Location::Session(session)));
}

#[test]
fn pausing_a_move_while_its_original_is_being_removed_lets_the_removal_finish() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    mark_running(&mut t, ids[0], 4000);
    t.engine.transfers.get_mut(ids[0]).unwrap().remove_source = true;
    t.engine.source_removals.insert(ids[0]);

    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    assert!(job(&t, ids[0]).paused);
    assert!(!t.engine.pause_stops.contains(&ids[0]));
    assert_eq!(job(&t, ids[0]).status, JobStatus::InProgress);

    t.engine.handle_transfer_event(TransferEvent::SourceRemoved { id: ids[0] });

    let done = job(&t, ids[0]);
    assert_eq!(done.status, JobStatus::Completed);
    assert!(!done.paused);
    assert!(t.engine.source_removals.is_empty());
}

#[test]
fn a_failed_removal_after_a_pause_clears_the_pause_and_the_stop_marker() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin"]);
    mark_running(&mut t, ids[0], 4000);
    t.engine.transfers.get_mut(ids[0]).unwrap().remove_source = true;
    t.engine.source_removals.insert(ids[0]);
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });

    t.engine.handle_transfer_event(TransferEvent::SourceRemovalFailed { id: ids[0], message: "denied".to_string() });

    let failed = job(&t, ids[0]);
    assert!(matches!(failed.status, JobStatus::Failed(_)));
    assert!(!failed.paused);
    assert!(t.engine.pause_stops.is_empty() && t.engine.source_removals.is_empty());
}

#[test]
fn a_cancelled_scope_with_no_pause_behind_it_still_cancels_the_job() {
    let (mut t, _remote, _batch, ids) = engine_with_idle_queue(&["a.bin"]);
    mark_running(&mut t, ids[0], 100);

    t.engine.handle_transfer_event(TransferEvent::Finished { id: ids[0], outcome: TransferOutcome::Cancelled });

    assert_eq!(job(&t, ids[0]).status, JobStatus::Cancelled);
}

#[test]
fn disconnecting_while_a_row_is_paused_fails_its_jobs_and_forgets_the_stops() {
    let (mut t, _remote, batch, ids) = engine_with_idle_queue(&["a.bin", "b.bin"]);
    mark_running(&mut t, ids[0], 100);
    t.engine.handle_command(Command::PauseRow { kind: RowKind::Batch(batch) });
    let session = t.engine.sessions.keys().next().copied().unwrap();

    t.engine.disconnect(session);

    assert!(t.engine.pause_stops.is_empty());
    assert!(matches!(job(&t, ids[1]).status, JobStatus::Failed(_)));
}

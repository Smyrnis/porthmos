use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use porthmos_core::transfer::{
    JobStatus, TransferQueue,
    rows::{RowState, ScanInfo},
};

use super::*;
use crate::app::testing::{TestApp, test_app};

fn app_on_transfers_screen() -> TestApp {
    let mut test = test_app(Path::new("/d"));
    test.app.apply_action(Action::OpenTransfers);
    test
}

fn add_job(queue: &mut TransferQueue, name: &str, batch_id: Option<u64>, status: JobStatus) -> u64 {
    let id = queue.enqueue(
        999,
        Direction::Upload,
        PathBuf::from(format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        10,
        batch_id,
    );
    queue.get_mut(id).unwrap().status = status;
    id
}

fn show(test: &mut TestApp, queue: &TransferQueue, scans: &[ScanInfo]) {
    test.app.apply_core_event(Event::TransfersChanged(TransferSnapshot::of(queue, scans)));
}

fn scan(batch_id: u64, label: &str) -> ScanInfo<'_> {
    ScanInfo { batch_id, label, direction: Direction::Upload, state: RowState::Scanning }
}

#[test]
fn ctrl_t_opens_the_transfers_screen_from_files_and_connections() {
    let mut test = app_on_transfers_screen();
    assert_eq!(test.app.screen, Screen::Transfers);

    test.app.screen = Screen::Connections;
    test.app.apply_action(Action::OpenTransfers);

    assert_eq!(test.app.screen, Screen::Transfers);
}

#[test]
fn esc_returns_to_files() {
    let mut test = app_on_transfers_screen();

    test.app.apply_action(Action::Back);

    assert_eq!(test.app.screen, Screen::Files);
}

#[test]
fn cancel_on_a_batch_row_cancels_only_that_batch() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("photos".to_string());
    add_job(&mut queue, "a", Some(batch), JobStatus::Queued);
    let other_batch = queue.start_batch("docs".to_string());
    add_job(&mut queue, "d", Some(other_batch), JobStatus::Queued);
    show(&mut test, &queue, &[]);

    test.app.apply_action(Action::Delete);

    assert_eq!(test.sent(), vec![Command::CancelRow { kind: RowKind::Batch(batch) }]);
}

#[test]
fn cancel_on_a_scan_row_flags_only_that_scan() {
    let mut test = app_on_transfers_screen();
    show(&mut test, &TransferQueue::new(), &[scan(1, "one"), scan(2, "two")]);
    test.app.transfers_cursor = 1;

    test.app.apply_action(Action::Delete);

    assert_eq!(test.sent(), vec![Command::CancelRow { kind: RowKind::Scan(2) }]);
}

#[test]
fn retry_asks_the_core_to_retry_the_selected_row() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    let failed = add_job(&mut queue, "a", None, JobStatus::Failed("boom".to_string()));
    show(&mut test, &queue, &[]);

    test.app.apply_action(Action::Open);

    assert_eq!(test.sent(), vec![Command::RetryRow { kind: RowKind::Single(failed) }]);
}

#[test]
fn clear_removes_only_finished_rows_and_clamps_the_cursor() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    add_job(&mut queue, "a", None, JobStatus::Completed);
    add_job(&mut queue, "b", None, JobStatus::InProgress);
    let batch = queue.start_batch("photos".to_string());
    add_job(&mut queue, "c", Some(batch), JobStatus::Failed("boom".to_string()));
    show(&mut test, &queue, &[]);
    test.app.transfers_cursor = 2;

    test.app.apply_action(Action::Refresh);
    let mut remaining = TransferQueue::new();
    add_job(&mut remaining, "b", None, JobStatus::InProgress);
    show(&mut test, &remaining, &[]);

    assert_eq!(test.sent(), vec![Command::ClearFinished]);
    assert_eq!(test.app.transfer_rows().len(), 1);
    assert_eq!(test.app.transfers_cursor, 0);
}

#[test]
fn clearing_keeps_the_cursor_on_the_same_row() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    add_job(&mut queue, "a", None, JobStatus::Completed);
    let selected = add_job(&mut queue, "b", None, JobStatus::InProgress);
    add_job(&mut queue, "c", None, JobStatus::InProgress);
    add_job(&mut queue, "d", None, JobStatus::Completed);
    show(&mut test, &queue, &[]);
    test.app.transfers_cursor = 1;

    test.app.apply_action(Action::Refresh);
    let finished: Vec<u64> = queue.jobs().filter(|job| job.status == JobStatus::Completed).map(|job| job.id).collect();
    queue.remove_jobs(&finished);
    show(&mut test, &queue, &[]);

    assert_eq!(test.app.transfer_rows()[test.app.transfers_cursor].kind, RowKind::Single(selected));
}

#[test]
fn actions_on_an_empty_list_do_nothing() {
    let mut test = app_on_transfers_screen();

    for action in [Action::Up, Action::Down, Action::Delete, Action::Open, Action::Refresh] {
        test.app.apply_action(action);
    }

    assert_eq!(test.app.transfers_cursor, 0);
    assert!(test.app.notifications.current().is_none());
    assert_eq!(test.app.screen, Screen::Transfers);
    assert!(test.sent().is_empty());
}

#[test]
fn up_and_down_stay_within_the_rows() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    add_job(&mut queue, "a", None, JobStatus::Queued);
    add_job(&mut queue, "b", None, JobStatus::Queued);
    show(&mut test, &queue, &[]);

    for _ in 0..3 {
        test.app.apply_action(Action::Down);
    }
    assert_eq!(test.app.transfers_cursor, 1);
    for _ in 0..3 {
        test.app.apply_action(Action::Up);
    }
    assert_eq!(test.app.transfers_cursor, 0);
}

#[test]
fn cursor_past_the_end_acts_on_the_last_row() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    let only = add_job(&mut queue, "a", None, JobStatus::Queued);
    show(&mut test, &queue, &[]);
    test.app.transfers_cursor = 5;

    test.app.apply_action(Action::Delete);

    assert_eq!(test.sent(), vec![Command::CancelRow { kind: RowKind::Single(only) }]);
}

#[test]
fn the_transfers_screen_renders_its_rows() {
    use ratatui::{Terminal, backend::TestBackend};

    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    add_job(&mut queue, "report.pdf", None, JobStatus::Queued);
    show(&mut test, &queue, &[]);
    let backend = TestBackend::new(100, 12);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| test.app.render(frame)).unwrap();

    let content: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
    assert!(content.contains("report.pdf"));
}

fn job_for(queue: &mut TransferQueue, session: u64, direction: Direction, name: &str, status: JobStatus) -> u64 {
    let id = queue.enqueue(
        session,
        direction,
        PathBuf::from(format!("/local/{name}")),
        format!("/remote/{name}"),
        name.to_string(),
        10,
        None,
    );
    queue.get_mut(id).unwrap().status = status;
    id
}

fn first_row(test: &TestApp) -> QueueRow {
    test.app.transfer_rows()[0].clone()
}

fn fixed(hour: u32, minute: u32) -> crate::clock::Clock {
    use chrono::TimeZone;
    crate::clock::Clock::fixed(chrono::Local.with_ymd_and_hms(2026, 10, 21, hour, minute, 0).unwrap())
}

#[test]
fn the_route_of_an_upload_names_the_session() {
    let mut test = app_on_transfers_screen();
    test.connect(5, "prod");
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 5, Direction::Upload, "a.txt", JobStatus::Queued);
    show(&mut test, &queue, &[]);

    assert_eq!(test.app.route_of(&first_row(&test)), "local \u{2192} prod");
}

#[test]
fn the_route_of_a_download_puts_the_session_first() {
    let mut test = app_on_transfers_screen();
    test.connect(5, "prod");
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 5, Direction::Download, "a.txt", JobStatus::Queued);
    show(&mut test, &queue, &[]);

    assert_eq!(test.app.route_of(&first_row(&test)), "prod \u{2192} local");
}

#[test]
fn a_route_survives_the_disconnect_of_its_session() {
    let mut test = app_on_transfers_screen();
    test.connect(5, "prod");
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 5, Direction::Upload, "a.txt", JobStatus::Completed);
    show(&mut test, &queue, &[]);

    test.app.sessions.remove(5);

    assert_eq!(test.app.route_of(&first_row(&test)), "local \u{2192} prod");
}

#[test]
fn a_scan_row_has_no_route() {
    let mut test = app_on_transfers_screen();
    show(&mut test, &TransferQueue::new(), &[scan(1, "folder")]);

    assert_eq!(test.app.route_of(&first_row(&test)), "");
}

#[test]
fn an_unknown_session_falls_back_to_remote() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 77, Direction::Upload, "a.txt", JobStatus::Queued);
    show(&mut test, &queue, &[]);

    assert_eq!(test.app.route_of(&first_row(&test)), "local \u{2192} remote");
}

#[test]
fn the_failed_time_is_the_clock_time_of_the_first_snapshot_showing_the_failure() {
    let mut test = app_on_transfers_screen();
    test.app.clock = fixed(14, 7);
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 1, Direction::Upload, "a.txt", JobStatus::Failed("boom".to_string()));

    show(&mut test, &queue, &[]);

    let kind = first_row(&test).kind;
    assert_eq!(test.app.failed_time_of(kind).map(|time| time.format("%H:%M").to_string()), Some("14:07".to_string()));
}

#[test]
fn a_later_snapshot_does_not_move_the_failed_time() {
    let mut test = app_on_transfers_screen();
    test.app.clock = fixed(14, 7);
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 1, Direction::Upload, "a.txt", JobStatus::Failed("boom".to_string()));
    show(&mut test, &queue, &[]);

    test.app.clock = fixed(15, 30);
    show(&mut test, &queue, &[]);

    let kind = first_row(&test).kind;
    assert_eq!(test.app.failed_time_of(kind).map(|time| time.format("%H:%M").to_string()), Some("14:07".to_string()));
}

#[test]
fn retrying_a_failed_row_forgets_its_failed_time() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    let id = job_for(&mut queue, 1, Direction::Upload, "a.txt", JobStatus::Failed("boom".to_string()));
    show(&mut test, &queue, &[]);
    let kind = first_row(&test).kind;

    queue.retry_jobs(&[id]);
    show(&mut test, &queue, &[]);

    assert!(test.app.failed_time_of(kind).is_none());
}

#[test]
fn a_cleared_row_is_forgotten() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 1, Direction::Upload, "a.txt", JobStatus::Failed("boom".to_string()));
    show(&mut test, &queue, &[]);
    let kind = first_row(&test).kind;

    show(&mut test, &TransferQueue::new(), &[]);

    assert!(test.app.failed_time_of(kind).is_none());
    assert!(test.app.failed_at.is_empty());
}

#[test]
fn a_partly_failed_row_also_gets_a_failed_time() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("folder".to_string());
    add_job(&mut queue, "a", Some(batch), JobStatus::Completed);
    add_job(&mut queue, "b", Some(batch), JobStatus::Failed("boom".to_string()));

    show(&mut test, &queue, &[]);

    assert!(test.app.failed_time_of(first_row(&test).kind).is_some());
}

#[test]
fn snapshots_feed_the_rate_tracker_without_panicking_on_empty_or_running_rows() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 1, Direction::Upload, "a.txt", JobStatus::InProgress);

    show(&mut test, &queue, &[]);
    show(&mut test, &TransferQueue::new(), &[]);

    assert_eq!(test.app.rates.aggregate_speed(), None);
}

#[test]
fn a_running_row_schedules_a_rate_wake_up_and_an_idle_queue_does_not() {
    let mut test = app_on_transfers_screen();
    assert!(test.app.next_rate_wake().is_none());
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 1, Direction::Upload, "a.txt", JobStatus::InProgress);

    show(&mut test, &queue, &[]);

    assert!(test.app.next_rate_wake().is_some());
    show(&mut test, &TransferQueue::new(), &[]);
    assert!(test.app.next_rate_wake().is_none());
}

#[test]
fn a_rate_wake_up_re_observes_the_rows_so_a_stalled_transfer_reports_zero() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    job_for(&mut queue, 1, Direction::Upload, "a.txt", JobStatus::InProgress);
    show(&mut test, &queue, &[]);
    let kind = first_row(&test).kind;
    let base = Instant::now();
    test.app.rates.observe(base, &test.app.transfers.rows.clone());

    test.app.tick_rates(base + Duration::from_secs(3));

    assert_eq!(test.app.rates.speed(kind), Some(0));
}

#[test]
fn r_on_the_transfers_screen_retries_like_enter_and_does_not_rename() {
    let outcome = |code: KeyCode| {
        let mut test = app_on_transfers_screen();
        let mut queue = TransferQueue::new();
        add_job(&mut queue, "a", None, JobStatus::Failed("boom".to_string()));
        show(&mut test, &queue, &[]);
        test.sent();
        test.app.apply_key(crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE));
        (test.sent(), test.app.dialog.is_some(), test.app.screen)
    };

    let by_letter = outcome(KeyCode::Char('r'));

    assert_eq!(by_letter, outcome(KeyCode::Enter));
    assert!(matches!(by_letter.0.as_slice(), [Command::RetryRow { .. }]));
    assert!(!by_letter.1);
}

#[test]
fn x_and_c_on_the_transfers_screen_cancel_and_clear() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    let batch = queue.start_batch("photos".to_string());
    add_job(&mut queue, "a", Some(batch), JobStatus::Queued);
    show(&mut test, &queue, &[]);
    test.sent();

    test.type_text("x");
    assert_eq!(test.sent(), vec![Command::CancelRow { kind: RowKind::Batch(batch) }]);

    test.type_text("c");
    assert!(test.app.dialog.is_none());
}

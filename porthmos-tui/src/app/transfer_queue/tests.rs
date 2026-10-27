use std::time::{Duration, Instant};

use porthmos_core::VfsPath;
use porthmos_core::transfer::{
    JobStatus, TransferQueue,
    rows::{RowState, ScanInfo},
};

use super::*;
use crate::app::testing::{TestApp, test_app};

fn app_on_transfers_screen() -> TestApp {
    let mut test = test_app(&VfsPath::new("/d"));
    test.app.apply_action(Action::OpenTransfers);
    test
}

fn add_job(queue: &mut TransferQueue, name: &str, batch_id: Option<u64>, status: JobStatus) -> u64 {
    let id = queue.enqueue(
        999,
        Direction::Upload,
        VfsPath::from(format!("/local/{name}")),
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
        VfsPath::from(format!("/local/{name}")),
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

fn dialog_text(test: &TestApp) -> Option<(String, String)> {
    match &test.app.dialog {
        Some(crate::widgets::dialog::Dialog::TextInput(dialog)) => Some((dialog.title.clone(), dialog.value.clone())),
        _ => None,
    }
}

fn type_limit(test: &mut TestApp, text: &str) {
    if let Some(crate::widgets::dialog::Dialog::TextInput(dialog)) = test.app.dialog.as_mut() {
        dialog.value = text.to_string();
    }
    test.app.apply_dialog_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    ));
}

fn snapshot_with_limit(limit: Option<u64>) -> Event {
    Event::TransfersChanged(TransferSnapshot { speed_limit: limit, ..TransferSnapshot::default() })
}

#[test]
fn the_speed_limit_dialog_starts_with_off_when_there_is_no_limit() {
    let mut test = app_on_transfers_screen();

    test.app.apply_action(Action::SpeedLimit);

    let (title, value) = dialog_text(&test).expect("a text dialog");
    assert!(title.contains("Speed limit"), "{title}");
    assert_eq!(value, "off");
}

#[test]
fn the_speed_limit_dialog_starts_with_the_current_limit_in_the_form_it_accepts() {
    let mut test = app_on_transfers_screen();
    test.app.apply_core_event(snapshot_with_limit(Some(2 * 1024 * 1024)));

    test.app.apply_action(Action::SpeedLimit);

    assert_eq!(dialog_text(&test).unwrap().1, "2M");
}

#[test]
fn the_speed_limit_can_be_set_from_any_screen() {
    let mut test = test_app(&VfsPath::new("/d"));

    test.app.apply_action(Action::SpeedLimit);

    assert!(dialog_text(&test).is_some());
}

#[test]
fn submitting_a_speed_sets_the_limit_and_says_so() {
    let mut test = app_on_transfers_screen();
    test.app.apply_action(Action::SpeedLimit);
    test.sent();

    type_limit(&mut test, "2M");

    assert_eq!(test.sent(), vec![Command::SetSpeedLimit { bytes_per_second: Some(2 * 1024 * 1024) }]);
    assert_eq!(test.notification().as_deref(), Some("Speed limit set to 2.0 MB/s"));
    assert!(test.app.dialog.is_none());
}

#[test]
fn submitting_off_removes_the_limit_and_says_so() {
    let mut test = app_on_transfers_screen();
    test.app.apply_core_event(snapshot_with_limit(Some(1000)));
    test.app.apply_action(Action::SpeedLimit);
    test.sent();

    type_limit(&mut test, "off");

    assert_eq!(test.sent(), vec![Command::SetSpeedLimit { bytes_per_second: None }]);
    assert_eq!(test.notification().as_deref(), Some("Speed limit off"));
}

#[test]
fn something_that_is_not_a_speed_sends_nothing_and_explains() {
    let mut test = app_on_transfers_screen();
    test.app.apply_action(Action::SpeedLimit);
    test.sent();

    type_limit(&mut test, "fast");

    assert!(test.sent().is_empty());
    assert_eq!(test.notification().as_deref(), Some("\"fast\" isn't a speed; try 500k, 2M or off"));
}

#[test]
fn cancelling_the_speed_limit_dialog_sends_nothing() {
    let mut test = app_on_transfers_screen();
    test.app.apply_action(Action::SpeedLimit);
    test.sent();

    test.app.apply_dialog_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    ));

    assert!(test.sent().is_empty());
    assert!(test.app.dialog.is_none());
}

fn showing_rows(test: &mut TestApp, states: &[RowState]) {
    let mut queue = TransferQueue::new();
    for (index, state) in states.iter().enumerate() {
        let status = match state {
            RowState::Running => JobStatus::InProgress,
            RowState::Done => JobStatus::Completed,
            _ => JobStatus::Queued,
        };
        let id = add_job(&mut queue, &format!("f{index}"), None, status);
        if *state == RowState::Paused {
            queue.get_mut(id).unwrap().paused = true;
        }
    }
    show(test, &queue, &[]);
}

fn press_pause(test: &mut TestApp, cursor: usize) -> Vec<Command> {
    test.app.transfers_cursor = cursor;
    test.sent();
    test.app.apply_action(Action::Pause);
    test.sent()
}

#[test]
fn p_pauses_a_running_or_queued_row() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Running, RowState::Queued]);
    let kinds: Vec<RowKind> = test.app.transfer_rows().iter().map(|row| row.kind).collect();

    assert_eq!(press_pause(&mut test, 0), vec![Command::PauseRow { kind: kinds[0] }]);
    assert_eq!(press_pause(&mut test, 1), vec![Command::PauseRow { kind: kinds[1] }]);
}

#[test]
fn p_on_a_paused_row_resumes_it() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Paused]);
    let kind = test.app.transfer_rows()[0].kind;

    assert_eq!(press_pause(&mut test, 0), vec![Command::ResumeRow { kind }]);
}

#[test]
fn p_on_a_finished_row_or_with_no_rows_sends_nothing() {
    let mut test = app_on_transfers_screen();
    assert!(press_pause(&mut test, 0).is_empty());

    showing_rows(&mut test, &[RowState::Done]);

    assert!(press_pause(&mut test, 0).is_empty());
}

#[test]
fn pausing_is_available_only_on_the_transfers_screen() {
    let mut test = test_app(&VfsPath::new("/d"));
    showing_rows(&mut test, &[RowState::Running]);

    test.sent();
    test.app.apply_action(Action::Pause);

    assert!(test.sent().is_empty());
}

fn press_reorder(test: &mut TestApp, cursor: usize, action: Action) -> Vec<Command> {
    test.app.transfers_cursor = cursor;
    test.sent();
    test.app.apply_action(action);
    test.sent()
}

#[test]
fn capital_k_moves_the_selected_row_earlier_and_capital_j_later() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Running, RowState::Queued, RowState::Paused]);
    let kinds: Vec<RowKind> = test.app.transfer_rows().iter().map(|row| row.kind).collect();

    assert_eq!(press_reorder(&mut test, 1, Action::RowUp), vec![Command::ReorderRow { kind: kinds[1], earlier: true }]);
    assert_eq!(
        press_reorder(&mut test, 1, Action::RowDown),
        vec![Command::ReorderRow { kind: kinds[1], earlier: false }]
    );
    assert_eq!(press_reorder(&mut test, 2, Action::RowUp), vec![Command::ReorderRow { kind: kinds[2], earlier: true }]);
}

#[test]
fn the_cursor_follows_the_row_to_its_new_place() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Queued, RowState::Queued, RowState::Queued]);
    let kinds: Vec<RowKind> = test.app.transfer_rows().iter().map(|row| row.kind).collect();
    press_reorder(&mut test, 2, Action::RowUp);

    let mut queue = TransferQueue::new();
    for name in ["f0", "f1", "f2"] {
        add_job(&mut queue, name, None, JobStatus::Queued);
    }
    queue.move_row(kinds[2], true);
    show(&mut test, &queue, &[]);

    assert_eq!(test.app.transfers_cursor, 1);
}

#[test]
fn reordering_a_finished_row_or_with_no_rows_sends_nothing() {
    let mut test = app_on_transfers_screen();
    assert!(press_reorder(&mut test, 0, Action::RowUp).is_empty());

    showing_rows(&mut test, &[RowState::Done]);

    assert!(press_reorder(&mut test, 0, Action::RowDown).is_empty());
}

#[test]
fn reordering_is_available_only_on_the_transfers_screen() {
    let mut test = test_app(&VfsPath::new("/d"));
    showing_rows(&mut test, &[RowState::Queued, RowState::Queued]);

    test.sent();
    test.app.apply_action(Action::RowUp);
    test.app.apply_action(Action::RowDown);

    assert!(test.sent().is_empty());
}

#[test]
fn moving_the_first_row_earlier_or_the_last_later_sends_nothing() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Queued, RowState::Queued]);

    assert!(press_reorder(&mut test, 0, Action::RowUp).is_empty());
    assert!(press_reorder(&mut test, 1, Action::RowDown).is_empty());
    assert!(test.app.reselect_row.is_none());
}

#[test]
fn finished_rows_and_scans_are_not_neighbours_for_reordering() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Done, RowState::Queued, RowState::Queued]);
    let kinds: Vec<RowKind> = test.app.transfer_rows().iter().map(|row| row.kind).collect();

    assert!(press_reorder(&mut test, 1, Action::RowUp).is_empty());
    assert_eq!(press_reorder(&mut test, 2, Action::RowUp), vec![Command::ReorderRow { kind: kinds[2], earlier: true }]);
}

#[test]
fn a_scan_row_after_the_last_row_is_not_a_target_for_moving_later() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    add_job(&mut queue, "f0", None, JobStatus::Queued);
    show(&mut test, &queue, &[scan(5, "scanning")]);

    assert!(press_reorder(&mut test, 0, Action::RowDown).is_empty());
    assert!(press_reorder(&mut test, 1, Action::RowUp).is_empty());
}

#[test]
fn moving_the_cursor_cancels_a_pending_follow() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Queued, RowState::Queued, RowState::Queued]);
    press_reorder(&mut test, 2, Action::RowUp);
    assert!(test.app.reselect_row.is_some());

    test.app.apply_action(Action::Up);

    assert!(test.app.reselect_row.is_none());
}

#[test]
fn leaving_and_reopening_the_transfers_screen_forgets_a_pending_follow() {
    let mut test = app_on_transfers_screen();
    showing_rows(&mut test, &[RowState::Queued, RowState::Queued, RowState::Queued]);
    press_reorder(&mut test, 2, Action::RowUp);
    assert!(test.app.reselect_row.is_some());

    test.app.apply_action(Action::OpenTransfers);

    assert!(test.app.reselect_row.is_none());
    assert_eq!(test.app.transfers_cursor, 0);
}

#[test]
fn p_on_a_running_row_that_is_still_stopping_resumes_it() {
    let mut test = app_on_transfers_screen();
    let mut queue = TransferQueue::new();
    let id = add_job(&mut queue, "f0", None, JobStatus::InProgress);
    queue.get_mut(id).unwrap().paused = true;
    show(&mut test, &queue, &[]);
    let row = test.app.transfer_rows()[0].clone();
    assert_eq!(row.state, RowState::Running);
    assert!(row.paused);

    assert_eq!(press_pause(&mut test, 0), vec![Command::ResumeRow { kind: row.kind }]);
}

#[test]
fn an_invalid_speed_reopens_the_dialog_with_what_was_typed_so_it_can_be_fixed() {
    let mut test = app_on_transfers_screen();
    test.app.apply_action(Action::SpeedLimit);
    test.sent();

    type_limit(&mut test, "2 fast");

    assert!(test.sent().is_empty());
    assert_eq!(dialog_text(&test).map(|(_, value)| value).as_deref(), Some("2 fast"));
    assert!(matches!(test.app.pending_action, Some(PendingAction::SpeedLimit)));

    type_limit(&mut test, "2M");

    assert_eq!(test.sent(), vec![Command::SetSpeedLimit { bytes_per_second: Some(2 * 1024 * 1024) }]);
    assert!(test.app.dialog.is_none());
}

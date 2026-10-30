use porthmos_core::transfer::rows::{QueueRow, RowKind, RowState};
use porthmos_core::transfer::{Direction, TransferSnapshot};
use porthmos_core::{Command, Entry, Location, VfsPath};

use super::{
    QUEUE_ROW_HEIGHT, QueuePane, RowButton, TransferState, failure_text, format_speed_limit, parse_speed_limit,
    queue_window, row_buttons, state_word, transfer_command,
};

fn row(state: RowState, bytes_done: u64, bytes_total: u64) -> QueueRow {
    QueueRow {
        kind: RowKind::Single(1),
        label: "file".to_string(),
        direction: Direction::Upload,
        files_done: 0,
        files_total: 1,
        bytes_done,
        bytes_total,
        state,
        paused: false,
        session_id: Some(1),
        failure: None,
    }
}

fn state_of(rows: Vec<QueueRow>, awaiting: usize) -> TransferState {
    let mut state = TransferState::default();
    state.replace(TransferSnapshot { rows, awaiting_answers: awaiting, ..TransferSnapshot::default() });
    state
}

fn entry(name: &str) -> Entry {
    Entry { name: name.to_string(), path: VfsPath::new("/src").join(name), ..Entry::default() }
}

#[test]
fn copy_builds_a_copy_command_with_the_destination_folder() {
    let command =
        transfer_command(false, (Location::Local, vec![entry("a")]), (Location::Session(2), VfsPath::new("/srv")));

    assert_eq!(
        command,
        Command::Copy {
            from: Location::Local,
            entries: vec![entry("a")],
            to: Location::Session(2),
            dest_dir: VfsPath::new("/srv")
        }
    );
}

#[test]
fn move_builds_a_move_command() {
    let command = transfer_command(
        true,
        (Location::Session(2), vec![entry("a"), entry("b")]),
        (Location::Local, VfsPath::new("/home")),
    );

    assert!(matches!(command, Command::Move { from: Location::Session(2), to: Location::Local, .. }));
}

#[test]
fn nothing_is_shown_before_the_first_snapshot() {
    let state = TransferState::default();

    assert_eq!(state.summary(), None);
    assert!(!state.has_unfinished());
}

#[test]
fn the_summary_counts_running_and_queued_rows_and_averages_progress() {
    let state =
        state_of(vec![row(RowState::Running, 20, 100), row(RowState::Queued, 0, 100), row(RowState::Done, 50, 50)], 0);

    assert_eq!(state.summary(), Some("2 transfers · 10%".to_string()));
    assert!(state.has_unfinished());
}

#[test]
fn one_transfer_is_singular() {
    let state = state_of(vec![row(RowState::Running, 50, 100)], 0);

    assert_eq!(state.summary(), Some("1 transfer · 50%".to_string()));
}

#[test]
fn a_scanning_row_without_a_size_shows_no_percent() {
    let state = state_of(vec![row(RowState::Scanning, 0, 0)], 0);

    assert_eq!(state.summary(), Some("1 transfer".to_string()));
}

#[test]
fn the_summary_is_none_when_everything_is_finished() {
    let state =
        state_of(vec![row(RowState::Done, 10, 10), row(RowState::Failed, 0, 10), row(RowState::Cancelled, 0, 10)], 0);

    assert_eq!(state.summary(), None);
    assert!(!state.has_unfinished());
}

#[test]
fn the_summary_mentions_conflicts_waiting_for_an_answer() {
    let state = state_of(vec![row(RowState::AwaitingAnswer, 0, 100)], 1);

    assert_eq!(state.summary(), Some("1 waiting for an answer".to_string()));
}

#[test]
fn running_rows_and_waiting_conflicts_are_both_reported() {
    let state = state_of(vec![row(RowState::Running, 50, 100), row(RowState::AwaitingAnswer, 0, 100)], 2);

    assert_eq!(state.summary(), Some("1 transfer · 50% · 2 waiting for an answer".to_string()));
}

#[test]
fn a_new_snapshot_replaces_the_old_one() {
    let mut state = state_of(vec![row(RowState::Running, 0, 100)], 0);

    state.replace(TransferSnapshot::default());

    assert_eq!(state.summary(), None);
}

fn failed_row(file: &str, message: &str, attempts: u32) -> QueueRow {
    QueueRow {
        failure: Some(porthmos_core::transfer::rows::RowFailure {
            file: file.to_string(),
            message: message.to_string(),
            attempts,
        }),
        ..row(RowState::Failed, 0, 10)
    }
}

#[test]
fn every_row_state_has_a_word_that_is_not_only_a_colour() {
    let words: Vec<&str> = [
        RowState::Scanning,
        RowState::AwaitingAnswer,
        RowState::Running,
        RowState::Queued,
        RowState::Paused,
        RowState::Done,
        RowState::PartlyFailed(2),
        RowState::Failed,
        RowState::Cancelled,
    ]
    .into_iter()
    .map(|state| state_word(&row(state, 0, 0)))
    .collect();

    assert_eq!(
        words,
        [
            "Scanning",
            "Waiting for an answer",
            "Running",
            "Queued",
            "Paused",
            "Done",
            "Partly failed",
            "Failed",
            "Cancelled"
        ]
    );
}

#[test]
fn a_row_flagged_paused_says_paused_even_while_queued() {
    let queued = QueueRow { paused: true, ..row(RowState::Queued, 0, 10) };

    assert_eq!(state_word(&queued), "Paused");
}

#[test]
fn a_failed_row_shows_its_file_attempts_and_message() {
    let text = failure_text(&failed_row("a.txt", "connection reset", 3)).expect("failure text");

    assert_eq!(text, "a.txt · 3 attempts · connection reset");
    assert_eq!(failure_text(&failed_row("a.txt", "denied", 1)).as_deref(), Some("a.txt · 1 attempt · denied"));
    assert_eq!(failure_text(&row(RowState::Running, 0, 1)), None);
}

fn buttons(state: RowState) -> Vec<RowButton> {
    row_buttons(&row(state, 0, 10))
}

#[test]
fn a_running_row_offers_pause_cancel_and_reordering() {
    assert_eq!(
        buttons(RowState::Running),
        [RowButton::Pause, RowButton::Cancel, RowButton::MoveUp, RowButton::MoveDown]
    );
}

#[test]
fn a_paused_row_offers_resume_not_pause() {
    let offered = buttons(RowState::Paused);

    assert!(offered.contains(&RowButton::Resume));
    assert!(!offered.contains(&RowButton::Pause));
}

#[test]
fn a_failed_row_offers_retry_and_a_finished_row_offers_nothing_else() {
    assert_eq!(buttons(RowState::Failed), [RowButton::Retry]);
    assert_eq!(buttons(RowState::PartlyFailed(1)), [RowButton::Retry]);
    assert!(buttons(RowState::Done).is_empty());
    assert!(buttons(RowState::Cancelled).is_empty());
}

#[test]
fn a_scan_or_a_waiting_row_can_only_be_cancelled() {
    assert_eq!(buttons(RowState::Scanning), [RowButton::Cancel]);
    assert_eq!(buttons(RowState::AwaitingAnswer), [RowButton::Cancel]);
}

#[test]
fn row_buttons_become_the_core_command_for_the_row_kind() {
    let kind = RowKind::Batch(7);

    assert_eq!(RowButton::Pause.command(kind), Command::PauseRow { kind });
    assert_eq!(RowButton::Resume.command(kind), Command::ResumeRow { kind });
    assert_eq!(RowButton::Cancel.command(kind), Command::CancelRow { kind });
    assert_eq!(RowButton::Retry.command(kind), Command::RetryRow { kind });
    assert_eq!(RowButton::MoveUp.command(kind), Command::ReorderRow { kind, earlier: true });
    assert_eq!(RowButton::MoveDown.command(kind), Command::ReorderRow { kind, earlier: false });
}

#[test]
fn parse_speed_limit_accepts_k_m_and_off_and_rejects_nonsense() {
    assert_eq!(parse_speed_limit("500K"), Ok(Some(500 * 1024)));
    assert_eq!(parse_speed_limit("2m"), Ok(Some(2 * 1024 * 1024)));
    assert_eq!(parse_speed_limit(" 1G "), Ok(Some(1024 * 1024 * 1024)));
    assert_eq!(parse_speed_limit("1500"), Ok(Some(1500)));
    assert_eq!(parse_speed_limit(""), Ok(None));
    assert_eq!(parse_speed_limit("off"), Ok(None));
    assert_eq!(parse_speed_limit("OFF"), Ok(None));
    for nonsense in ["fast", "-5", "1.5M", "0", "0K", "M", "12X", "99999999999999999999G"] {
        assert!(parse_speed_limit(nonsense).is_err(), "{nonsense}");
    }
}

#[test]
fn a_speed_limit_is_written_the_way_it_is_typed() {
    assert_eq!(format_speed_limit(None), "off");
    assert_eq!(format_speed_limit(Some(500 * 1024)), "500K");
    assert_eq!(format_speed_limit(Some(2 * 1024 * 1024)), "2M");
    assert_eq!(format_speed_limit(Some(1500)), "1500");
    for limit in [Some(1500), Some(3 * 1024), Some(7 * 1024 * 1024), None] {
        assert_eq!(parse_speed_limit(&format_speed_limit(limit)), Ok(limit));
    }
}

#[test]
fn the_queue_window_holds_only_the_rows_near_the_viewport() {
    let window = queue_window(1000, 0.0);

    assert!(window.len() < 20, "{window:?}");
    assert_eq!(window.start, 0);
    let scrolled = queue_window(1000, 500.0 * QUEUE_ROW_HEIGHT);
    assert!(scrolled.contains(&500));
    assert!(scrolled.len() < 20);
    assert_eq!(queue_window(0, 0.0), 0..0);
}

#[test]
fn the_pane_opens_for_a_new_transfer_and_stays_closed_once_dismissed() {
    let mut pane = QueuePane::default();

    pane.snapshot_changed(true);
    assert!(pane.is_open());

    pane.toggle(true);
    assert!(!pane.is_open());
    pane.snapshot_changed(true);
    assert!(!pane.is_open());

    pane.snapshot_changed(false);
    pane.snapshot_changed(true);
    assert!(pane.is_open());
}

#[test]
fn closing_an_idle_pane_does_not_dismiss_the_next_batch() {
    let mut pane = QueuePane::default();
    pane.toggle(false);
    assert!(pane.is_open());
    pane.toggle(false);
    assert!(!pane.is_open());

    pane.snapshot_changed(true);

    assert!(pane.is_open());
}

#[test]
fn opening_the_pane_by_hand_undoes_the_dismissal() {
    let mut pane = QueuePane::default();
    pane.snapshot_changed(true);
    pane.toggle(true);
    pane.toggle(true);

    pane.snapshot_changed(true);

    assert!(pane.is_open());
}

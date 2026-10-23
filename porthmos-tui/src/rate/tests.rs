use std::time::{Duration, Instant};

use porthmos_core::transfer::{
    Direction,
    rows::{QueueRow, RowKind, RowState},
};

use super::*;

fn row(id: u64, bytes_done: u64, bytes_total: u64, state: RowState) -> QueueRow {
    QueueRow {
        kind: RowKind::Single(id),
        label: format!("file{id}"),
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

fn running(id: u64, done: u64, total: u64) -> QueueRow {
    row(id, done, total, RowState::Running)
}

fn at(base: Instant, seconds: u64) -> Instant {
    base + Duration::from_secs(seconds)
}

const MB: u64 = 1_000_000;

#[test]
fn no_speed_before_two_samples() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    tracker.observe(base, &[running(1, 0, 10 * MB)]);

    assert_eq!(tracker.speed(RowKind::Single(1)), None);
    assert_eq!(tracker.aggregate_speed(), None);
}

#[test]
fn steady_progress_gives_bytes_per_second() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    for second in 0..=4 {
        tracker.observe(at(base, second), &[running(1, second * MB, 100 * MB)]);
    }

    assert_eq!(tracker.speed(RowKind::Single(1)), Some(MB));
}

#[test]
fn a_burst_outside_the_window_is_forgotten() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    tracker.observe(base, &[running(1, 0, 100 * MB)]);
    tracker.observe(at(base, 1), &[running(1, 50 * MB, 100 * MB)]);
    for second in 7..=11 {
        tracker.observe(at(base, second), &[running(1, 50 * MB + (second - 7) * MB, 100 * MB)]);
    }

    assert_eq!(tracker.speed(RowKind::Single(1)), Some(MB));
}

#[test]
fn a_stalled_transfer_reports_zero_and_no_eta() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    tracker.observe(base, &[running(1, 5 * MB, 10 * MB)]);
    tracker.observe(at(base, 3), &[running(1, 5 * MB, 10 * MB)]);

    assert_eq!(tracker.speed(RowKind::Single(1)), Some(0));
    assert_eq!(tracker.eta_seconds(&running(1, 5 * MB, 10 * MB)), None);
}

#[test]
fn a_drop_in_bytes_after_a_retry_clears_the_window() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    tracker.observe(base, &[running(1, 0, 10 * MB)]);
    tracker.observe(at(base, 2), &[running(1, 8 * MB, 10 * MB)]);
    tracker.observe(at(base, 3), &[running(1, 0, 10 * MB)]);

    assert_eq!(tracker.speed(RowKind::Single(1)), None);
    tracker.observe(at(base, 4), &[running(1, MB, 10 * MB)]);
    assert_eq!(tracker.speed(RowKind::Single(1)), Some(MB));
}

#[test]
fn samples_at_the_same_instant_give_no_speed() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    tracker.observe(base, &[running(1, 0, 10 * MB)]);
    tracker.observe(base, &[running(1, MB, 10 * MB)]);

    assert_eq!(tracker.speed(RowKind::Single(1)), None);
}

#[test]
fn a_row_that_disappears_is_forgotten() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();
    tracker.observe(base, &[running(1, 0, 10 * MB)]);
    tracker.observe(at(base, 1), &[running(1, MB, 10 * MB)]);

    tracker.observe(at(base, 2), &[]);

    assert_eq!(tracker.speed(RowKind::Single(1)), None);
    assert_eq!(tracker.aggregate_speed(), None);
}

#[test]
fn rows_that_are_not_running_are_not_tracked() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    for second in 0..3 {
        tracker.observe(at(base, second), &[row(1, second * MB, 10 * MB, RowState::Queued)]);
    }
    tracker.observe(at(base, 3), &[row(1, 3 * MB, 10 * MB, RowState::Done)]);

    assert_eq!(tracker.speed(RowKind::Single(1)), None);
}

#[test]
fn eta_is_remaining_bytes_over_speed() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();
    tracker.observe(base, &[running(1, 0, 100 * MB)]);
    tracker.observe(at(base, 2), &[running(1, 20 * MB, 100 * MB)]);

    assert_eq!(tracker.eta_seconds(&running(1, 20 * MB, 100 * MB)), Some(8));
    assert_eq!(tracker.eta_seconds(&running(2, 0, 100 * MB)), None);
}

#[test]
fn bytes_beyond_the_total_give_an_eta_of_zero_and_an_empty_total_gives_none() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();
    tracker.observe(base, &[running(1, 0, MB), running(2, 0, 0)]);
    tracker.observe(at(base, 1), &[running(1, 2 * MB, MB), running(2, MB, 0)]);

    assert_eq!(tracker.eta_seconds(&running(1, 2 * MB, MB)), Some(0));
    assert_eq!(tracker.eta_seconds(&running(2, MB, 0)), None);
}

#[test]
fn aggregate_speed_sums_running_rows() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();
    tracker.observe(base, &[running(1, 0, 100 * MB), running(2, 0, 100 * MB)]);
    tracker.observe(at(base, 1), &[running(1, MB, 100 * MB), running(2, 3 * MB, 100 * MB)]);

    assert_eq!(tracker.aggregate_speed(), Some(4 * MB));
}

#[test]
fn ten_thousand_rows_observe_in_linear_time() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();
    let started = Instant::now();

    for second in 0..50 {
        let rows: Vec<QueueRow> = (0..10_000).map(|id| running(id, second * 1000, 1_000_000)).collect();
        tracker.observe(at(base, second), &rows);
    }

    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
    assert_eq!(tracker.speed(RowKind::Single(9_999)), Some(1000));
}

#[test]
fn format_speed_units() {
    assert_eq!(format_speed(0), "0 B/s");
    assert_eq!(format_speed(812), "812 B/s");
    assert_eq!(format_speed(1024), "1.0 KB/s");
    assert_eq!(format_speed(13_002_342), "12.4 MB/s");
    assert!(!format_speed(u64::MAX).is_empty());
}

#[test]
fn format_eta_units() {
    assert_eq!(format_eta(0), "0:00");
    assert_eq!(format_eta(41), "0:41");
    assert_eq!(format_eta(59), "0:59");
    assert_eq!(format_eta(60), "1:00");
    assert_eq!(format_eta(725), "12:05");
    assert_eq!(format_eta(3599), "59:59");
    assert_eq!(format_eta(3600), "1:00:00");
    assert_eq!(format_eta(3723), "1:02:03");
    assert_eq!(format_eta(u64::MAX), "99:59:59");
    assert_eq!(format_eta(360_000), "99:59:59");
}

#[test]
fn a_slow_transfer_keeps_enough_samples_to_report_a_speed() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();

    tracker.observe(base, &[running(1, 0, 100)]);
    tracker.observe(at(base, 20), &[running(1, 10, 100)]);

    assert_eq!(tracker.speed(RowKind::Single(1)), Some(0));
    tracker.observe(at(base, 40), &[running(1, 20, 100)]);
    assert_eq!(tracker.speed(RowKind::Single(1)), Some(0));
}

#[test]
fn re_observing_the_same_bytes_later_turns_a_frozen_speed_into_zero() {
    let base = Instant::now();
    let mut tracker = RateTracker::default();
    for second in 0..=4 {
        tracker.observe(at(base, second), &[running(1, second * MB, 100 * MB)]);
    }
    assert_eq!(tracker.speed(RowKind::Single(1)), Some(MB));

    for second in 5..=12 {
        tracker.observe(at(base, second), &[running(1, 4 * MB, 100 * MB)]);
    }

    assert_eq!(tracker.speed(RowKind::Single(1)), Some(0));
}

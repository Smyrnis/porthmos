use std::sync::atomic::AtomicI64;

use super::*;

fn total(counter: &StderrCounter) -> usize {
    counter.shared().load(Ordering::Relaxed)
}

#[test]
fn a_find_that_exits_cleanly_is_complete_and_counts_no_unreadable_folders() {
    let exit = AtomicI64::new(0);

    assert!(find_completed(&exit));
    assert_eq!(find_unreadable(&exit, 0), 0);
}

#[test]
fn a_clean_exit_ignores_what_was_printed_on_stderr() {
    let exit = AtomicI64::new(0);

    assert_eq!(find_unreadable(&exit, 4), 0);
}

#[test]
fn a_find_that_could_not_read_some_folders_counts_the_lines_it_complained_about() {
    let exit = AtomicI64::new(FIND_PARTIAL_FAILURE);

    assert!(find_completed(&exit));
    assert_eq!(find_unreadable(&exit, 3), 3);
}

#[test]
fn a_partial_failure_without_any_stderr_lines_counts_at_least_one() {
    let exit = AtomicI64::new(FIND_PARTIAL_FAILURE);

    assert_eq!(find_unreadable(&exit, 0), 1);
}

#[test]
fn a_find_whose_exit_status_never_arrived_counts_nothing() {
    let exit = AtomicI64::new(EXIT_UNKNOWN);

    assert!(!find_completed(&exit));
    assert_eq!(find_unreadable(&exit, 5), 0);
}

#[test]
fn stderr_lines_are_counted_across_chunks() {
    let mut counter = StderrCounter::default();

    counter.push(b"find: '/a': Permission den");
    counter.push(b"ied\nfind: '/b': Permission denied\nfind: '/c'");
    counter.push(b": Permission denied\n");

    assert_eq!(total(&counter), 3);
}

#[test]
fn a_last_stderr_line_without_a_newline_is_counted_when_the_stream_ends() {
    let mut counter = StderrCounter::default();

    counter.push(b"find: '/a': Permission denied\nfind: '/b': Permission denied");
    assert_eq!(total(&counter), 1);
    counter.finish();

    assert_eq!(total(&counter), 2);
}

#[test]
fn blank_stderr_lines_are_not_counted() {
    let mut counter = StderrCounter::default();

    counter.push(b"\n\nfind: '/a': Permission denied\n\n");
    counter.finish();

    assert_eq!(total(&counter), 1);
}

#[test]
fn the_counter_reads_through_a_shared_handle_so_the_search_can_use_it_after_the_stream_ends() {
    let mut counter = StderrCounter::default();
    let shared = counter.shared();

    counter.push(b"find: '/a': Permission denied\n");

    assert_eq!(shared.load(Ordering::Relaxed), 1);
}

#[test]
fn only_find_complaints_count_not_warnings_or_other_output() {
    let mut counter = StderrCounter::default();

    counter.push(b"find: warning: you have specified the global option -maxdepth after the argument -iname\n");
    counter.push(b"find: '/a': Permission denied\n");
    counter.push(b"some wrapper noise\n");
    counter.push(b"find: /b: Permission denied\n");

    assert_eq!(total(&counter), 2);
}

#[test]
fn a_last_line_that_is_not_a_find_complaint_is_not_counted_at_the_end_either() {
    let mut counter = StderrCounter::default();

    counter.push(b"Connection to host closed");
    counter.finish();

    assert_eq!(total(&counter), 0);
}

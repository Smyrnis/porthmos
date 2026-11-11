use std::sync::atomic::AtomicI64;

use super::*;

#[test]
fn a_find_that_exits_cleanly_is_complete() {
    let exit = AtomicI64::new(0);

    assert!(find_completed(&exit));
    assert!(!find_incomplete(&exit));
}

#[test]
fn a_find_that_could_not_read_some_folders_is_finished_but_incomplete() {
    let exit = AtomicI64::new(FIND_PARTIAL_FAILURE);

    assert!(find_completed(&exit));
    assert!(find_incomplete(&exit));
}

#[test]
fn a_find_whose_exit_status_never_arrived_is_not_called_incomplete() {
    let exit = AtomicI64::new(EXIT_UNKNOWN);

    assert!(!find_completed(&exit));
    assert!(!find_incomplete(&exit));
}

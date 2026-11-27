use super::{Fetches, first_line};

#[test]
fn begin_returns_none_while_in_flight() {
    let mut fetches = Fetches::<u8>::default();

    assert!(fetches.begin(1).is_some());

    assert!(fetches.begin(1).is_none());
    assert!(fetches.begin(2).is_some());
}

#[test]
fn finish_clears_in_flight_so_the_next_begin_succeeds() {
    let mut fetches = Fetches::<u8>::default();
    let first = fetches.begin(1).unwrap();

    assert!(fetches.finish(1, first).is_some());
    let second = fetches.begin(1).unwrap();

    assert_ne!(first, second);
}

#[test]
fn a_stale_generation_finishes_to_none() {
    let mut fetches = Fetches::<u8>::default();
    let first = fetches.begin(1).unwrap();
    fetches.finish(1, first);
    let second = fetches.begin(1).unwrap();

    assert!(fetches.finish(1, first).is_none());
    assert!(fetches.begin(1).is_none());
    assert!(fetches.finish(1, second).is_some());
}

#[test]
fn stop_makes_the_old_generation_stale() {
    let mut fetches = Fetches::<u8>::default();
    let old = fetches.begin(1).unwrap();

    fetches.stop(1);

    assert!(!fetches.has(1));
    assert!(fetches.finish(1, old).is_none());
    let fresh = fetches.begin(1).unwrap();
    assert_ne!(old, fresh);
    assert!(fetches.finish(1, old).is_none());
    assert!(fetches.has(1));
}

#[test]
fn a_queued_refetch_is_reported_once_and_only_for_the_current_run() {
    let mut fetches = Fetches::<u8>::default();
    let first = fetches.begin(1).unwrap();

    assert!(fetches.queue_refetch(1));
    assert!(fetches.queue_refetch(1));
    assert!(fetches.finish(1, first).is_some());
    assert!(fetches.take_refetch(1));
    assert!(!fetches.take_refetch(1));

    assert!(!fetches.queue_refetch(1));
    let second = fetches.begin(1).unwrap();
    assert!(fetches.queue_refetch(1));
    fetches.stop(1);
    assert!(fetches.finish(1, second).is_none());
    assert!(!fetches.take_refetch(1));
    assert!(!fetches.queue_refetch(2));
}

#[test]
fn first_line_drops_controls_and_text_that_hides_or_reorders() {
    let hostile = "\u{202e}fo\u{200b}o\u{1b}[31m bar\n".as_bytes();

    assert_eq!(first_line(hostile).as_deref(), Some("foo[31m bar"));
    assert_eq!(first_line("\n\u{200b}\n  \nreal".as_bytes()).as_deref(), Some("real"));
}

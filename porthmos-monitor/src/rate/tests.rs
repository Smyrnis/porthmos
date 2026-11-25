use std::time::Duration;

use super::*;

#[test]
fn a_rate_is_the_delta_over_the_seconds_elapsed() {
    assert_eq!(per_second(4000, 1000, Duration::from_secs(3)), Some(1000));
    assert_eq!(per_second(1000, 1000, Duration::from_secs(3)), Some(0));
    assert_eq!(per_second(1500, 1000, Duration::from_millis(500)), Some(1000));
}

#[test]
fn a_counter_that_went_backwards_or_no_elapsed_time_gives_no_rate() {
    assert_eq!(per_second(10, 1000, Duration::from_secs(3)), None);
    assert_eq!(per_second(4000, 1000, Duration::ZERO), None);
}

#[test]
fn huge_counters_do_not_overflow() {
    assert_eq!(per_second(u64::MAX, 0, Duration::from_secs(1)), Some(u64::MAX));
    assert!(per_second(u64::MAX, 0, Duration::from_nanos(1)).is_some());
}

use chrono::{Duration, Local, TimeZone};

use super::*;

#[test]
fn a_fixed_clock_always_returns_its_time() {
    let moment = Local.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap();
    let clock = Clock::fixed(moment);

    assert_eq!(clock.now(), moment);
    assert_eq!(clock.now(), moment);
}

#[test]
fn the_system_clock_is_close_to_now() {
    let before = Local::now();
    let seen = Clock::system().now();
    let after = Local::now();

    assert!(seen >= before - Duration::seconds(1));
    assert!(seen <= after + Duration::seconds(1));
}

#[test]
fn the_default_clock_is_the_system_clock() {
    let seen = Clock::default().now();

    assert!((Local::now() - seen).num_seconds().abs() < 5);
}

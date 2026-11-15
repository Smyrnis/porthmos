use std::time::{Duration, Instant};

use super::*;

#[test]
fn push_makes_the_message_current() {
    let mut notifications = Notifications::default();
    notifications.push(Severity::Info, "saved");
    assert_eq!(notifications.current().unwrap().message, "saved");
}

#[test]
fn earlier_pushes_stay_current_until_dismissed_fifo() {
    let mut notifications = Notifications::default();
    notifications.push(Severity::Info, "first");
    notifications.push(Severity::Info, "second");

    assert_eq!(notifications.current().unwrap().message, "first");
    notifications.dismiss_current();
    assert_eq!(notifications.current().unwrap().message, "second");
}

#[test]
fn expire_drops_only_notifications_whose_deadline_has_passed() {
    let now = Instant::now();
    let mut notifications = Notifications::default();
    notifications.queue.push_back(Notification {
        severity: Severity::Info,
        message: "old".to_string(),
        expires_at: now,
    });
    notifications.queue.push_back(Notification {
        severity: Severity::Info,
        message: "fresh".to_string(),
        expires_at: now + Duration::from_secs(10),
    });

    notifications.expire(now + Duration::from_millis(1));

    assert_eq!(notifications.current().unwrap().message, "fresh");
}

#[test]
fn every_severity_lasts_as_long_as_core_says() {
    for severity in [Severity::Info, Severity::Warning, Severity::Error] {
        let lifetime = severity.lifetime();
        let mut notifications = Notifications::default();
        let before = Instant::now();
        notifications.push(severity, "message");
        let after = Instant::now();

        let wake = notifications.next_wake().unwrap();

        assert!(wake >= before + lifetime && wake <= after + lifetime, "{severity:?}");
    }
}

#[test]
fn an_error_stays_until_its_lifetime_has_passed_and_then_goes() {
    let mut notifications = Notifications::default();
    notifications.push(Severity::Error, "connection failed");
    let wake = notifications.next_wake().unwrap();

    notifications.expire(wake - Duration::from_millis(1));
    assert_eq!(notifications.current().unwrap().message, "connection failed");

    notifications.expire(wake);
    assert!(notifications.current().is_none());
}

#[test]
fn next_wake_is_none_when_the_queue_is_empty() {
    assert_eq!(Notifications::default().next_wake(), None);
}

#[test]
fn a_repeated_error_gets_a_fresh_lifetime() {
    let mut notifications = Notifications::default();
    notifications.push(Severity::Error, "boom");
    let first = notifications.next_wake().unwrap();
    std::thread::sleep(Duration::from_millis(5));

    notifications.push(Severity::Error, "boom");

    assert!(notifications.next_wake().unwrap() > first);
}

#[test]
fn next_wake_returns_the_front_entrys_deadline() {
    let mut notifications = Notifications::default();
    notifications.push(Severity::Warning, "careful");
    assert!(notifications.next_wake().is_some());
}

#[test]
fn identical_consecutive_messages_collapse_into_one_with_a_count() {
    let mut notifications = Notifications::default();
    for _ in 0..12 {
        notifications.push(Severity::Error, "Transfer failed: Permission denied");
    }

    assert_eq!(notifications.current().unwrap().message, "Transfer failed: Permission denied (x12)");
    notifications.dismiss_current();
    assert!(notifications.current().is_none());
}

#[test]
fn messages_separated_by_another_message_do_not_collapse() {
    let mut notifications = Notifications::default();
    notifications.push(Severity::Error, "a");
    notifications.push(Severity::Error, "b");
    notifications.push(Severity::Error, "a");

    let mut seen = Vec::new();
    while let Some(current) = notifications.current() {
        seen.push(current.message.clone());
        notifications.dismiss_current();
    }
    assert_eq!(seen, ["a", "b", "a"]);
}

#[test]
fn the_queue_is_bounded_and_keeps_the_newest_messages() {
    let mut notifications = Notifications::default();
    for n in 0..1000 {
        notifications.push(Severity::Error, format!("failure {n}"));
    }

    let mut seen = Vec::new();
    while let Some(current) = notifications.current() {
        seen.push(current.message.clone());
        notifications.dismiss_current();
    }
    assert!(seen.len() <= 50, "{} notifications stayed queued", seen.len());
    assert_eq!(seen.last().map(String::as_str), Some("failure 999"));
}

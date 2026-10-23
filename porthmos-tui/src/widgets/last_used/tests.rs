use chrono::{Duration, Local, TimeZone};

use super::*;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, 12, 0, 0).unwrap()
}

fn ago(duration: Duration) -> Option<DateTime<Utc>> {
    Some(now() - duration)
}

fn short(duration: Duration) -> String {
    last_used_text(now(), ago(duration))
}

#[test]
fn a_connection_that_was_never_used_says_never() {
    assert_eq!(last_used_text(now(), None), "never");
    assert_eq!(last_used_detail(now(), None), "never");
}

#[test]
fn under_a_minute_is_just_now() {
    assert_eq!(short(Duration::zero()), "just now");
    assert_eq!(short(Duration::seconds(59)), "just now");
}

#[test]
fn a_time_a_little_ahead_of_the_clock_is_just_now() {
    assert_eq!(short(Duration::seconds(-30)), "just now");
    assert_eq!(short(Duration::seconds(-59)), "just now");
}

#[test]
fn a_time_well_ahead_of_the_clock_is_shown_as_its_date() {
    let at = now() + Duration::days(40);

    assert_eq!(last_used_text(now(), Some(at)), at.with_timezone(&Local).format("%Y-%m-%d").to_string());
    assert_eq!(short(Duration::minutes(-2)), now().with_timezone(&Local).format("%Y-%m-%d").to_string());
}

#[test]
fn the_detail_of_a_time_ahead_of_the_clock_says_so() {
    let at = now() + Duration::days(40);

    assert_eq!(
        last_used_detail(now(), Some(at)),
        format!("{} \u{b7} in the future", at.with_timezone(&Local).format("%Y-%m-%d %H:%M"))
    );
}

#[test]
fn minutes_run_up_to_the_hour() {
    assert_eq!(short(Duration::seconds(60)), "1 min ago");
    assert_eq!(short(Duration::minutes(3)), "3 min ago");
    assert_eq!(short(Duration::seconds(59 * 60 + 59)), "59 min ago");
}

#[test]
fn hours_run_up_to_the_day() {
    assert_eq!(short(Duration::hours(1)), "1 h ago");
    assert_eq!(short(Duration::seconds(23 * 3600 + 3599)), "23 h ago");
}

#[test]
fn the_second_day_is_yesterday() {
    assert_eq!(short(Duration::hours(24)), "yesterday");
    assert_eq!(short(Duration::seconds(48 * 3600 - 1)), "yesterday");
}

#[test]
fn days_run_to_thirty() {
    assert_eq!(short(Duration::hours(48)), "2 d ago");
    assert_eq!(short(Duration::days(12)), "12 d ago");
    assert_eq!(short(Duration::days(30)), "30 d ago");
    assert_eq!(short(Duration::days(31) - Duration::seconds(1)), "30 d ago");
}

#[test]
fn after_thirty_days_it_is_the_local_date() {
    let at = now() - Duration::days(31);

    assert_eq!(last_used_text(now(), Some(at)), at.with_timezone(&Local).format("%Y-%m-%d").to_string());
}

#[test]
fn no_short_form_is_wider_than_the_date() {
    let widest = [
        Duration::seconds(5),
        Duration::minutes(59),
        Duration::hours(23),
        Duration::hours(30),
        Duration::days(30),
        Duration::days(400),
    ]
    .into_iter()
    .map(|age| short(age).chars().count())
    .max()
    .unwrap();

    assert!(widest <= 10, "{widest}");
}

#[test]
fn the_detail_gives_the_local_date_and_time_with_the_age() {
    let at = now() - Duration::hours(3);

    assert_eq!(
        last_used_detail(now(), Some(at)),
        format!("{} \u{b7} 3 h ago", at.with_timezone(&Local).format("%Y-%m-%d %H:%M"))
    );
}

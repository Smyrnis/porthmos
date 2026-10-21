use chrono::{DateTime, FixedOffset, TimeZone};

use super::*;

fn zone(hours: i32) -> FixedOffset {
    FixedOffset::east_opt(hours * 3600).unwrap()
}

fn now_at(hours: i32, year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> DateTime<FixedOffset> {
    zone(hours).with_ymd_and_hms(year, month, day, hour, minute, second).unwrap()
}

fn now() -> DateTime<FixedOffset> {
    now_at(3, 2026, 10, 20, 14, 30, 0)
}

fn stamp(hours: i32, year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Option<u64> {
    Some(now_at(hours, year, month, day, hour, minute, second).timestamp() as u64)
}

#[test]
fn none_is_an_empty_cell() {
    assert_eq!(format_modified(None, &now()), "");
    assert_eq!(format_modified_with_time(None, &now()), "");
}

#[test]
fn the_same_local_day_is_today_even_a_second_after_midnight() {
    assert_eq!(format_modified(stamp(3, 2026, 10, 20, 0, 0, 1), &now()), "today");
    assert_eq!(format_modified(stamp(3, 2026, 10, 20, 14, 30, 0), &now()), "today");
}

#[test]
fn a_second_before_midnight_is_yesterday() {
    assert_eq!(format_modified(stamp(3, 2026, 10, 19, 23, 59, 59), &now()), "yesterday");
}

#[test]
fn two_days_ago_shows_the_month_and_day() {
    assert_eq!(format_modified(stamp(3, 2026, 10, 18, 9, 0, 0), &now()), "Oct 18");
}

#[test]
fn a_single_digit_day_is_zero_padded() {
    assert_eq!(format_modified(stamp(3, 2026, 9, 2, 9, 0, 0), &now()), "Sep 02");
}

#[test]
fn new_years_day_sees_december_31_as_yesterday() {
    let new_year = now_at(0, 2027, 1, 1, 8, 0, 0);

    assert_eq!(format_modified(stamp(0, 2026, 12, 31, 23, 0, 0), &new_year), "yesterday");
    assert_eq!(format_modified(stamp(0, 2026, 12, 30, 23, 0, 0), &new_year), "Dec 30 2026");
}

#[test]
fn the_same_date_a_year_ago_shows_the_year() {
    let text = format_modified(stamp(3, 2025, 10, 20, 14, 0, 0), &now());

    assert_eq!(text, "Oct 20 2025");
    assert_eq!(text.len(), 11);
}

#[test]
fn a_future_timestamp_shows_its_date() {
    assert_eq!(format_modified(stamp(3, 2026, 10, 21, 1, 0, 0), &now()), "Oct 21");
    assert_eq!(format_modified(stamp(3, 2027, 10, 21, 1, 0, 0), &now()), "Oct 21 2027");
}

#[test]
fn the_epoch_and_extreme_values_never_panic() {
    assert_eq!(format_modified(Some(0), &now()), "Jan 01 1970");
    assert_eq!(format_modified(Some(u64::MAX), &now()), "");
    assert_eq!(format_modified(Some(1 << 40), &now()), "");
    assert_eq!(format_modified_with_time(Some(u64::MAX), &now()), "");
}

#[test]
fn the_file_date_uses_the_clock_time_zone() {
    let instant = stamp(0, 2026, 10, 19, 23, 0, 0);

    assert_eq!(format_modified(instant, &now_at(0, 2026, 10, 20, 12, 0, 0)), "yesterday");
    assert_eq!(format_modified(instant, &now_at(10, 2026, 10, 20, 12, 0, 0)), "today");
}

#[test]
fn with_time_appends_hours_and_minutes_for_recent_days() {
    assert_eq!(format_modified_with_time(stamp(3, 2026, 10, 20, 14, 2, 0), &now()), "today 14:02");
    assert_eq!(format_modified_with_time(stamp(3, 2026, 10, 19, 9, 5, 0), &now()), "yesterday 09:05");
    assert_eq!(format_modified_with_time(stamp(3, 2026, 9, 22, 14, 2, 0), &now()), "Sep 22 14:02");
}

#[test]
fn with_time_leaves_other_years_without_time() {
    assert_eq!(format_modified_with_time(stamp(3, 2025, 9, 22, 14, 2, 0), &now()), "Sep 22 2025");
}

#[test]
fn timestamps_near_the_largest_the_calendar_supports_never_panic() {
    let ahead = now_at(10, 2026, 10, 20, 12, 0, 0);

    for seconds in [8_210_266_876_799 - 60, 8_210_266_876_799, 253_402_300_799, 253_402_300_800] {
        assert_eq!(format_modified(Some(seconds), &ahead), "", "{seconds}");
        assert_eq!(format_modified_with_time(Some(seconds), &ahead), "", "{seconds}");
    }
}

use chrono::FixedOffset;

use super::{modified, size};

#[test]
fn sizes_under_a_kilobyte_are_raw_bytes() {
    assert_eq!(size(0, false), "0");
    assert_eq!(size(1023, false), "1023");
}

#[test]
fn sizes_use_binary_units_with_one_decimal() {
    assert_eq!(size(1024, false), "1.0K");
    assert_eq!(size(1536, false), "1.5K");
    assert_eq!(size(5 * 1024 * 1024, false), "5.0M");
    assert_eq!(size(3 * 1024 * 1024 * 1024, false), "3.0G");
    assert_eq!(size(u64::MAX, false), "16777216.0T");
}

#[test]
fn a_folder_has_no_size() {
    assert_eq!(size(4096, true), "—");
}

#[test]
fn modified_shows_date_and_minutes_in_the_given_zone() {
    let zone = FixedOffset::east_opt(2 * 3600).unwrap();

    assert_eq!(modified(Some(1_700_000_000), &zone), "2023-11-15 00:13");
}

#[test]
fn a_missing_or_absurd_modified_time_is_blank() {
    let zone = FixedOffset::east_opt(0).unwrap();

    assert_eq!(modified(None, &zone), "");
    assert_eq!(modified(Some(u64::MAX), &zone), "");
    assert_eq!(modified(Some(300_000_000_000), &zone), "");
}

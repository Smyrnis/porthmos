use std::fmt::Display;

use chrono::{DateTime, Datelike, TimeZone};

const LAST_SECOND_OF_YEAR_9999: i64 = 253_402_300_799;
const MAX_YEAR: i32 = 9999;

fn in_clock_zone<Tz: TimeZone>(modified: Option<u64>, now: &DateTime<Tz>) -> Option<DateTime<Tz>> {
    let seconds = i64::try_from(modified?).ok().filter(|seconds| *seconds <= LAST_SECOND_OF_YEAR_9999)?;
    let file = DateTime::from_timestamp(seconds, 0)?.with_timezone(&now.timezone());
    (file.year() <= MAX_YEAR).then_some(file)
}

enum Day {
    Today,
    Yesterday,
    ThisYear,
    OtherYear,
}

fn day_of<Tz: TimeZone>(file: &DateTime<Tz>, now: &DateTime<Tz>) -> Day {
    let today = now.date_naive();
    let date = file.date_naive();
    if date == today {
        Day::Today
    } else if today.pred_opt() == Some(date) {
        Day::Yesterday
    } else if date.year() == today.year() {
        Day::ThisYear
    } else {
        Day::OtherYear
    }
}

pub fn format_modified<Tz: TimeZone>(modified: Option<u64>, now: &DateTime<Tz>) -> String
where
    Tz::Offset: Display,
{
    let Some(file) = in_clock_zone(modified, now) else { return String::new() };
    match day_of(&file, now) {
        Day::Today => "today".to_string(),
        Day::Yesterday => "yesterday".to_string(),
        Day::ThisYear => file.format("%b %d").to_string(),
        Day::OtherYear => file.format("%b %d %Y").to_string(),
    }
}

pub fn format_modified_with_time<Tz: TimeZone>(modified: Option<u64>, now: &DateTime<Tz>) -> String
where
    Tz::Offset: Display,
{
    let Some(file) = in_clock_zone(modified, now) else { return String::new() };
    let label = format_modified(modified, now);
    match day_of(&file, now) {
        Day::OtherYear => label,
        _ => format!("{label} {}", file.format("%H:%M")),
    }
}

#[cfg(test)]
mod tests;

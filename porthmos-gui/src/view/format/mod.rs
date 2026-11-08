use std::fmt::Display;

use chrono::{DateTime, TimeZone};
use porthmos_core::size_text;

const LAST_SECOND_OF_YEAR_9999: i64 = 253_402_300_799;

pub fn size(bytes: u64, is_dir: bool) -> String {
    if is_dir { "—".to_string() } else { size_text(bytes) }
}

pub fn modified<Tz: TimeZone>(modified: Option<u64>, zone: &Tz) -> String
where
    Tz::Offset: Display,
{
    let seconds = modified.and_then(|seconds| i64::try_from(seconds).ok()).filter(|s| *s <= LAST_SECOND_OF_YEAR_9999);
    match seconds.and_then(|seconds| DateTime::from_timestamp(seconds, 0)) {
        Some(moment) => moment.with_timezone(zone).format("%Y-%m-%d %H:%M").to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests;

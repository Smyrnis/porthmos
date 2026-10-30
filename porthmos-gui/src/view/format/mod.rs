use std::fmt::Display;

use chrono::{DateTime, TimeZone};

const LAST_SECOND_OF_YEAR_9999: i64 = 253_402_300_799;
const UNITS: [&str; 4] = ["K", "M", "G", "T"];

pub fn size(bytes: u64, is_dir: bool) -> String {
    if is_dir {
        return "—".to_string();
    }
    if bytes < 1024 {
        return bytes.to_string();
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1}{}", UNITS[unit])
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

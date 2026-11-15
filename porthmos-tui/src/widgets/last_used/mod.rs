use chrono::{DateTime, Duration, Local, Utc};
use porthmos_core::i18n;

const SHORT_DAYS: i64 = 31;

fn is_ahead_of_the_clock(age: Duration) -> bool {
    age < -Duration::minutes(1)
}

fn age_text(age: Duration) -> String {
    if is_ahead_of_the_clock(age) {
        i18n::text("lastused.future")
    } else if age < Duration::minutes(1) {
        i18n::text("lastused.just_now")
    } else if age < Duration::hours(1) {
        i18n::text_with("lastused.min", &[("count", &age.num_minutes().to_string())])
    } else if age < Duration::days(1) {
        i18n::text_with("lastused.hour", &[("count", &age.num_hours().to_string())])
    } else if age < Duration::days(2) {
        i18n::text("lastused.yesterday")
    } else {
        i18n::text_with("lastused.day", &[("count", &age.num_days().to_string())])
    }
}

pub fn last_used_text(now: DateTime<Utc>, at: Option<DateTime<Utc>>) -> String {
    let Some(at) = at else { return i18n::text("lastused.never") };
    let age = now - at;
    if age >= Duration::days(SHORT_DAYS) || is_ahead_of_the_clock(age) {
        return at.with_timezone(&Local).format("%Y-%m-%d").to_string();
    }
    age_text(age)
}

pub fn last_used_detail(now: DateTime<Utc>, at: Option<DateTime<Utc>>) -> String {
    let Some(at) = at else { return i18n::text("lastused.never") };
    format!("{} \u{b7} {}", at.with_timezone(&Local).format("%Y-%m-%d %H:%M"), age_text(now - at))
}

#[cfg(test)]
mod tests;

use chrono::{DateTime, Duration, Local, Utc};

const NEVER: &str = "never";
const JUST_NOW: &str = "just now";
const SHORT_DAYS: i64 = 31;

fn is_ahead_of_the_clock(age: Duration) -> bool {
    age < -Duration::minutes(1)
}

fn age_text(age: Duration) -> String {
    if is_ahead_of_the_clock(age) {
        "in the future".to_string()
    } else if age < Duration::minutes(1) {
        JUST_NOW.to_string()
    } else if age < Duration::hours(1) {
        format!("{} min ago", age.num_minutes())
    } else if age < Duration::days(1) {
        format!("{} h ago", age.num_hours())
    } else if age < Duration::days(2) {
        "yesterday".to_string()
    } else {
        format!("{} d ago", age.num_days())
    }
}

pub fn last_used_text(now: DateTime<Utc>, at: Option<DateTime<Utc>>) -> String {
    let Some(at) = at else { return NEVER.to_string() };
    let age = now - at;
    if age >= Duration::days(SHORT_DAYS) || is_ahead_of_the_clock(age) {
        return at.with_timezone(&Local).format("%Y-%m-%d").to_string();
    }
    age_text(age)
}

pub fn last_used_detail(now: DateTime<Utc>, at: Option<DateTime<Utc>>) -> String {
    let Some(at) = at else { return NEVER.to_string() };
    format!("{} \u{b7} {}", at.with_timezone(&Local).format("%Y-%m-%d %H:%M"), age_text(now - at))
}

#[cfg(test)]
mod tests;

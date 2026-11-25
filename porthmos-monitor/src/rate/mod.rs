use std::time::Duration;

pub(crate) fn per_second(now: u64, before: u64, elapsed: Duration) -> Option<u64> {
    let delta = now.checked_sub(before)?;
    let seconds = elapsed.as_secs_f64();
    (seconds > 0.0).then(|| (delta as f64 / seconds) as u64)
}

#[cfg(test)]
mod tests;

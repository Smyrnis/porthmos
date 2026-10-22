use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::time::Instant;

pub const MAX_CHUNK: usize = 256 * 1024;
const WAIT_SLICE: Duration = Duration::from_millis(200);
const CHUNKS_PER_SECOND: u64 = 10;

#[derive(Default)]
struct Inner {
    rate: AtomicU64,
    generation: AtomicU64,
    next_free: Mutex<Option<Instant>>,
}

#[derive(Clone, Default)]
pub struct SpeedLimit(Arc<Inner>);

impl SpeedLimit {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn with(limit: Option<u64>) -> Self {
        let speed = Self::new();
        speed.set(limit);
        speed
    }

    pub fn get(&self) -> Option<u64> {
        Some(self.0.rate.load(Ordering::Relaxed)).filter(|rate| *rate > 0)
    }

    pub fn set(&self, limit: Option<u64>) {
        let mut next = self.timeline();
        self.0.rate.store(limit.unwrap_or(0), Ordering::Relaxed);
        self.0.generation.fetch_add(1, Ordering::Relaxed);
        *next = None;
    }

    pub fn chunk_cap(&self) -> usize {
        match self.get() {
            None => MAX_CHUNK,
            Some(rate) => (rate / CHUNKS_PER_SECOND).clamp(1, MAX_CHUNK as u64) as usize,
        }
    }

    pub async fn throttle(&self, bytes: u64, cancel: &AtomicBool) -> bool {
        loop {
            let Some((start, cost, generation)) = self.reserve(bytes) else {
                return true;
            };
            loop {
                if cancel.load(Ordering::Relaxed) {
                    self.give_back(cost, generation);
                    return false;
                }
                let now = Instant::now();
                if now >= start {
                    return true;
                }
                if self.0.generation.load(Ordering::Relaxed) != generation {
                    break;
                }
                tokio::time::sleep((start - now).min(WAIT_SLICE)).await;
            }
        }
    }

    fn reserve(&self, bytes: u64) -> Option<(Instant, Duration, u64)> {
        let mut next = self.timeline();
        let rate = self.0.rate.load(Ordering::Relaxed);
        if rate == 0 {
            return None;
        }
        let generation = self.0.generation.load(Ordering::Relaxed);
        let billed = bytes.min((rate / CHUNKS_PER_SECOND).clamp(1, MAX_CHUNK as u64));
        let cost = Duration::from_secs_f64(billed as f64 / rate as f64);
        let now = Instant::now();
        let start = next.map_or(now, |free| free.max(now));
        *next = Some(start + cost);
        Some((start, cost, generation))
    }

    fn give_back(&self, cost: Duration, generation: u64) {
        let mut next = self.timeline();
        if self.0.generation.load(Ordering::Relaxed) != generation {
            return;
        }
        if let Some(free) = *next {
            *next = Some(free.checked_sub(cost).unwrap_or(free));
        }
    }

    fn timeline(&self) -> std::sync::MutexGuard<'_, Option<Instant>> {
        self.0.next_free.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub fn parse_speed_limit(text: &str) -> Result<Option<u64>, String> {
    let shown = text.trim();
    let lowered = shown.to_lowercase();
    if matches!(lowered.as_str(), "" | "off" | "none" | "no") {
        return Ok(None);
    }
    let invalid = || format!("\"{shown}\" isn't a speed; try 500k, 2M or off");
    let too_small = || format!("\"{shown}\" is below the minimum; a limit must be at least 1 byte a second");
    let too_large = || format!("\"{shown}\" is too large for a speed limit");

    let rest = lowered.strip_suffix("/s").unwrap_or(&lowered).trim_end();
    let rest = rest.strip_suffix('b').unwrap_or(rest);
    let rest = match rest.strip_suffix('i') {
        Some(before) if before.ends_with(['k', 'm', 'g']) => before,
        _ => rest,
    };
    let (number, shift) = match rest.chars().last() {
        Some('k') => (&rest[..rest.len() - 1], 10),
        Some('m') => (&rest[..rest.len() - 1], 20),
        Some('g') => (&rest[..rest.len() - 1], 30),
        _ => (rest, 0),
    };
    let number = number.trim();
    let well_formed = !number.is_empty()
        && number.chars().all(|character| character.is_ascii_digit() || character == '.')
        && number.matches('.').count() <= 1
        && number.chars().any(|character| character.is_ascii_digit());
    if !well_formed {
        return Err(invalid());
    }
    if !number.contains('.') {
        let whole = number.parse::<u64>().map_err(|_| too_large())?;
        return match whole.checked_mul(1u64 << shift).ok_or_else(too_large)? {
            0 => Ok(None),
            bytes => Ok(Some(bytes)),
        };
    }
    let value = number.parse::<f64>().map_err(|_| invalid())? * (1u64 << shift) as f64;
    if !value.is_finite() || value >= u64::MAX as f64 {
        return Err(too_large());
    }
    match value.floor() as u64 {
        0 if value == 0.0 => Ok(None),
        0 => Err(too_small()),
        bytes => Ok(Some(bytes)),
    }
}

pub fn format_speed_limit(limit: Option<u64>) -> String {
    let Some(limit) = limit else {
        return "off".to_string();
    };
    for (shift, unit) in [(30, "G"), (20, "M"), (10, "k")] {
        let size = 1u64 << shift;
        if limit % size == 0 {
            return format!("{}{unit}", limit / size);
        }
    }
    limit.to_string()
}

#[cfg(test)]
mod tests;

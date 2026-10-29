use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub use porthmos_core::Severity;

const MAX_QUEUED: usize = 20;

fn ttl_for(severity: Severity) -> Option<Duration> {
    match severity {
        Severity::Info => Some(Duration::from_secs(4)),
        Severity::Warning => Some(Duration::from_secs(8)),
        Severity::Error => None,
    }
}

fn without_count(message: &str) -> (&str, usize) {
    message
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" (x"))
        .and_then(|(base, count)| Some((base, count.parse::<usize>().ok()?)))
        .unwrap_or((message, 1))
}

pub struct Notification {
    pub severity: Severity,
    pub message: String,
    expires_at: Option<Instant>,
}

#[derive(Default)]
pub struct Notifications {
    queue: VecDeque<Notification>,
}

impl Notifications {
    pub fn push(&mut self, severity: Severity, message: impl Into<String>) {
        let message = message.into();
        let expires_at = ttl_for(severity).map(|ttl| Instant::now() + ttl);
        if let Some(last) = self.queue.back_mut()
            && last.severity == severity
            && without_count(&last.message).0 == message
        {
            let repeats = without_count(&last.message).1 + 1;
            last.message = format!("{message} (x{repeats})");
            last.expires_at = expires_at;
            return;
        }
        if self.queue.len() >= MAX_QUEUED {
            self.queue.remove(1);
        }
        self.queue.push_back(Notification { severity, message, expires_at });
    }

    pub fn current(&self) -> Option<&Notification> {
        self.queue.front()
    }

    pub fn dismiss_current(&mut self) {
        self.queue.pop_front();
    }

    pub fn expire(&mut self, now: Instant) {
        while let Some(front) = self.queue.front() {
            match front.expires_at {
                Some(at) if at <= now => {
                    self.queue.pop_front();
                }
                _ => break,
            }
        }
    }

    pub fn next_wake(&self) -> Option<Instant> {
        self.queue.front().and_then(|n| n.expires_at)
    }
}

#[cfg(test)]
mod tests;

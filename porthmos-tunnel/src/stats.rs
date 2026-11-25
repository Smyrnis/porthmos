use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatsSnapshot {
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub connections: u64,
    pub active: usize,
    pub errors: u64,
    pub refused: u64,
}

#[derive(Default)]
pub(crate) struct Counters {
    pub(crate) bytes_up: AtomicU64,
    pub(crate) bytes_down: AtomicU64,
    pub(crate) connections: AtomicU64,
    pub(crate) active: AtomicUsize,
    pub(crate) errors: AtomicU64,
    pub(crate) refused: AtomicU64,
}

#[derive(Clone, Default)]
pub(crate) struct Stats(Arc<Counters>);

impl Deref for Stats {
    type Target = Counters;

    fn deref(&self) -> &Counters {
        &self.0
    }
}

impl Stats {
    pub(crate) fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            bytes_up: self.bytes_up.load(Ordering::Relaxed),
            bytes_down: self.bytes_down.load(Ordering::Relaxed),
            connections: self.connections.load(Ordering::Relaxed),
            active: self.active.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            refused: self.refused.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn open(&self) -> ActiveGuard {
        self.connections.fetch_add(1, Ordering::Relaxed);
        self.active.fetch_add(1, Ordering::Relaxed);
        ActiveGuard(self.clone())
    }
}

pub(crate) struct ActiveGuard(Stats);

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Relaxed);
    }
}

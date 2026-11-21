use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

use super::{ConflictReview, PlanningScan, handlers};
use crate::transfer::{TransferSnapshot, conflicts::ConflictPolicy, limit::SpeedLimit};

pub(crate) struct TransferWork {
    pub(crate) planning: Vec<PlanningScan>,
    pub(crate) moves: HashMap<u64, handlers::MoveBatch>,
    pub(crate) reviews: VecDeque<ConflictReview>,
    pub(crate) pause_stops: HashSet<u64>,
    pub(crate) source_removals: HashSet<u64>,
    pub(crate) speed_limit: SpeedLimit,
    pub(crate) published: TransferSnapshot,
    pub(crate) last_progress_publish: Option<Instant>,
    pub(crate) publish_interval: Duration,
    pub(crate) transfers_dirty: bool,
    pub(crate) max_parallel: usize,
    pub(crate) on_conflict: ConflictPolicy,
}

impl TransferWork {
    pub(crate) fn new(publish_interval: Duration, max_parallel: usize, on_conflict: ConflictPolicy) -> Self {
        Self {
            planning: Vec::new(),
            moves: HashMap::new(),
            reviews: VecDeque::new(),
            pause_stops: HashSet::new(),
            source_removals: HashSet::new(),
            speed_limit: SpeedLimit::new(),
            published: TransferSnapshot::default(),
            last_progress_publish: None,
            publish_interval,
            transfers_dirty: false,
            max_parallel,
            on_conflict,
        }
    }
}

#[cfg(test)]
mod tests;

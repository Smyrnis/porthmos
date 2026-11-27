use std::{collections::HashMap, sync::Arc};

use porthmos_monitor::{POLL_TIMEOUT, SHELL_COMMAND, change_script, hides_or_reorders_text};
use porthmos_vfs::{CommandOutput, CommandRunner};
use tokio::time::timeout;

use super::super::SessionId;
use crate::{Message, i18n};

const REASON_LIMIT: usize = 200;

pub(crate) const FAILURES_BEFORE_NOTICE: u32 = 3;

pub(crate) struct Slot<T> {
    pub latest: Option<T>,
    pub unavailable: Option<Message>,
    pub failures: u32,
}

struct Entry<T> {
    generation: u64,
    in_flight: bool,
    refetch_after: bool,
    slot: Slot<T>,
}

pub(crate) struct Fetches<T> {
    entries: HashMap<SessionId, Entry<T>>,
    next_generation: u64,
}

impl<T> Default for Fetches<T> {
    fn default() -> Self {
        Self { entries: HashMap::new(), next_generation: 0 }
    }
}

impl<T> Fetches<T> {
    pub(crate) fn begin(&mut self, session: SessionId) -> Option<u64> {
        let entry = self.entries.entry(session).or_insert_with(|| Entry {
            generation: 0,
            in_flight: false,
            refetch_after: false,
            slot: Slot { latest: None, unavailable: None, failures: 0 },
        });
        if entry.in_flight {
            return None;
        }
        entry.in_flight = true;
        entry.generation = self.next_generation;
        self.next_generation += 1;
        Some(entry.generation)
    }

    pub(crate) fn current(&mut self, session: SessionId, generation: u64) -> Option<&mut Slot<T>> {
        self.entries
            .get_mut(&session)
            .filter(|entry| entry.in_flight && entry.generation == generation)
            .map(|entry| &mut entry.slot)
    }

    pub(crate) fn finish(&mut self, session: SessionId, generation: u64) -> Option<&mut Slot<T>> {
        let entry = self.entries.get_mut(&session).filter(|entry| entry.in_flight && entry.generation == generation)?;
        entry.in_flight = false;
        Some(&mut entry.slot)
    }

    pub(crate) fn queue_refetch(&mut self, session: SessionId) -> bool {
        let Some(entry) = self.entries.get_mut(&session).filter(|entry| entry.in_flight) else {
            return false;
        };
        entry.refetch_after = true;
        true
    }

    pub(crate) fn take_refetch(&mut self, session: SessionId) -> bool {
        self.entries.get_mut(&session).is_some_and(|entry| std::mem::take(&mut entry.refetch_after))
    }

    pub(crate) fn stop(&mut self, session: SessionId) {
        self.entries.remove(&session);
    }

    pub(crate) fn has(&self, session: SessionId) -> bool {
        self.entries.contains_key(&session)
    }

    pub(crate) fn slot(&self, session: SessionId) -> Option<&Slot<T>> {
        self.entries.get(&session).map(|entry| &entry.slot)
    }
}

pub(crate) fn first_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| {
            line.chars()
                .filter(|character| !character.is_control() && !hides_or_reorders_text(*character))
                .collect::<String>()
        })
        .map(|line| line.trim().chars().take(REASON_LIMIT).collect::<String>())
        .find(|line| !line.is_empty())
}

pub(crate) async fn run_change(runner: Arc<dyn CommandRunner>, command: String) -> Result<CommandOutput, String> {
    match timeout(POLL_TIMEOUT, runner.run_with_input(SHELL_COMMAND, change_script(&command).as_bytes())).await {
        Err(_) => Err(i18n::text("monitor.reason_timeout")),
        Ok(Err(error)) => Err(error.to_string()),
        Ok(Ok(output)) => Ok(output),
    }
}

#[cfg(test)]
mod tests;

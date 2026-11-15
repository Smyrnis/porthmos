use std::{collections::HashMap, time::Duration};

use super::super::{Engine, Internal, SessionId};
use crate::{Severity, tasks::Scope};

pub(crate) const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
    Duration::from_secs(40),
];

#[derive(Default)]
pub(crate) struct ReconnectState {
    pub(crate) enabled: bool,
    attempts: HashMap<String, u32>,
}

impl ReconnectState {
    pub(crate) fn new(enabled: bool) -> Self {
        Self { enabled, ..Self::default() }
    }

    fn attempt(&self, name: &str) -> Option<u32> {
        self.attempts.get(name).copied()
    }

    fn forget(&mut self, name: &str) -> bool {
        self.attempts.remove(name).is_some()
    }

    #[cfg(test)]
    fn is_idle(&self) -> bool {
        self.attempts.is_empty()
    }
}

impl Engine {
    pub(crate) fn begin_reconnect(&mut self, name: String) {
        if self.reconnect.enabled && self.reconnect.attempt(&name).is_none() {
            self.schedule_reconnect(name, 0);
        }
    }

    fn schedule_reconnect(&mut self, name: String, attempt: u32) {
        let Some(delay) = RETRY_DELAYS.get(attempt as usize).copied() else {
            self.reconnect.forget(&name);
            self.report(Severity::Warning, format!("{name}: could not reconnect \u{2014} connect again by hand"));
            return;
        };
        self.reconnect.attempts.insert(name.clone(), attempt);
        let internal = self.internal.clone();
        self.tasks.spawn("reconnect-timer", Scope::Background, move |_| async move {
            tokio::time::sleep(delay).await;
            let _ = internal.send(Internal::ReconnectDue { name, attempt });
        });
    }

    pub(crate) fn set_reconnect(&mut self, enabled: bool) {
        self.reconnect.enabled = enabled;
        if !enabled {
            self.reconnect.attempts.clear();
        }
    }

    pub(crate) fn reconnect_due(&mut self, name: String, attempt: u32) {
        if self.reconnect.attempt(&name) != Some(attempt) {
            return;
        }
        let live = self.sessions.iter().find(|(_, live)| live.name == name).map(|(&session, _)| session);
        match live {
            Some(session) if self.lost_sessions.contains(&session) => self.disconnect(session),
            Some(_) => {
                self.reconnect.forget(&name);
                return;
            }
            None => {}
        }
        if self.connecting.contains(&name) {
            self.reconnect.forget(&name);
            return;
        }
        self.info(format!("Reconnecting to {name}"));
        self.connect(&name);
        if !self.connecting.contains(&name) {
            self.reconnect.forget(&name);
        }
    }

    pub(crate) fn reconnect_succeeded(&mut self, name: &str) {
        if self.reconnect.forget(name) {
            self.info(format!("{name}: reconnected"));
        }
    }

    pub(crate) fn reconnect_failed(&mut self, name: &str, retryable: bool) {
        let Some(attempt) = self.reconnect.attempt(name) else {
            return;
        };
        if retryable {
            self.schedule_reconnect(name.to_string(), attempt + 1);
        } else {
            self.reconnect.forget(name);
        }
    }

    pub(crate) fn stop_reconnecting(&mut self, name: &str) {
        self.reconnect.forget(name);
    }

    pub(crate) fn stop_reconnecting_session(&mut self, session: SessionId) {
        if let Some(name) = self.sessions.get(&session).map(|live| live.name.clone()) {
            self.reconnect.forget(&name);
        }
    }
}

#[cfg(test)]
mod tests;

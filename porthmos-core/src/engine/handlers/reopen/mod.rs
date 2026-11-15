use std::collections::VecDeque;

use super::super::{Engine, Event};
use crate::config::ReopenMode;

#[derive(Default)]
pub(crate) struct ReopenState {
    pub(crate) mode: ReopenMode,
    offer: Vec<String>,
    queue: VecDeque<String>,
    current: Option<String>,
}

impl ReopenState {
    pub(crate) fn new(mode: ReopenMode) -> Self {
        Self { mode, ..Self::default() }
    }

    fn forget_pending(&mut self) {
        self.offer.clear();
        self.queue.clear();
        self.current = None;
    }
}

impl Engine {
    fn open_connection_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for live in self.sessions.values() {
            if !names.contains(&live.name) {
                names.push(live.name.clone());
            }
        }
        names
    }

    pub(crate) fn remember_open_connections(&mut self) {
        let names = if self.reopen.mode == ReopenMode::Off { Vec::new() } else { self.open_connection_names() };
        if self.ui_state.reopen == names {
            return;
        }
        self.ui_state.reopen = names;
        self.save_quietly("open connections");
    }

    pub(crate) fn offer_reopen(&mut self) {
        if self.reopen.mode == ReopenMode::Off {
            return;
        }
        let remembered = self.ui_state.reopen.clone();
        let names: Vec<String> = match self.all_profiles() {
            Ok(entries) => remembered
                .into_iter()
                .filter(|name| entries.iter().any(|entry| entry.name == *name && !entry.source.is_orphan_labels()))
                .collect(),
            Err(_) => remembered,
        };
        if names.is_empty() {
            return;
        }
        if self.reopen.mode == ReopenMode::Auto {
            self.start_reopening(names);
            return;
        }
        self.reopen.offer = names.clone();
        self.emit(Event::ReopenOffer { names });
    }

    pub(crate) fn reopen_connections(&mut self) {
        let names = std::mem::take(&mut self.reopen.offer);
        self.start_reopening(names);
    }

    pub(crate) fn set_reopen_mode(&mut self, mode: ReopenMode) {
        self.reopen.mode = mode;
        if mode == ReopenMode::Off {
            self.reopen.forget_pending();
        }
        self.remember_open_connections();
    }

    pub(crate) fn reopen_progressed(&mut self, name: &str) {
        if self.reopen.current.as_deref() == Some(name) {
            self.reopen.current = None;
            self.reopen_next();
        }
    }

    fn start_reopening(&mut self, names: Vec<String>) {
        if self.reopen.current.is_some() {
            self.reopen.queue.extend(names);
            return;
        }
        self.reopen.queue = names.into();
        self.reopen_next();
    }

    fn reopen_next(&mut self) {
        while let Some(name) = self.reopen.queue.pop_front() {
            self.connect(&name);
            if self.connecting.contains(&name) {
                self.reopen.current = Some(name);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests;

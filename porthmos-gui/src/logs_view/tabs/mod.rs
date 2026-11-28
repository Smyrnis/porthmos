use porthmos_core::{LogId, SessionId, i18n};

use super::{LogsState, SourceKind};

pub const MAX_LOG_TABS: usize = 6;

pub fn tabs_full_text() -> String {
    i18n::text_with("gui.logs.tabs_full", &[("count", &MAX_LOG_TABS.to_string())])
}

#[derive(Debug)]
pub struct LogTabs {
    tabs: Vec<LogsState>,
    active: usize,
    next_id: LogId,
}

impl Default for LogTabs {
    fn default() -> Self {
        Self { tabs: vec![LogsState::default()], active: 0, next_id: 1 }
    }
}

impl LogTabs {
    pub fn active(&self) -> &LogsState {
        &self.tabs[self.active]
    }

    pub fn active_mut(&mut self) -> &mut LogsState {
        &mut self.tabs[self.active]
    }

    pub fn tabs(&self) -> &[LogsState] {
        &self.tabs
    }

    pub fn tab_mut(&mut self, index: usize) -> Option<&mut LogsState> {
        self.tabs.get_mut(index)
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn can_add(&self) -> bool {
        self.tabs.len() < MAX_LOG_TABS
    }

    pub fn add(&mut self) -> bool {
        if !self.can_add() {
            return false;
        }
        self.tabs.push(LogsState::default());
        self.active = self.tabs.len() - 1;
        true
    }

    pub fn select(&mut self, index: usize) -> bool {
        let Some(tab) = self.tabs.get_mut(index) else {
            return false;
        };
        tab.was_visible = false;
        self.active = index;
        true
    }

    pub fn close(&mut self, index: usize) -> Option<LogId> {
        if index >= self.tabs.len() {
            return None;
        }
        let removed = self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.tabs.push(LogsState::default());
            self.active = 0;
        } else if index < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len() - 1);
            self.tabs[self.active].was_visible = false;
        }
        removed.running.map(|(id, _)| id)
    }

    pub fn take_id(&mut self) -> LogId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn by_id_mut(&mut self, id: LogId) -> Option<&mut LogsState> {
        self.tabs.iter_mut().find(|tab| tab.running.map(|(running, _)| running) == Some(id))
    }

    pub fn following(&self, unit: &str, session: SessionId) -> Option<usize> {
        self.tabs.iter().position(|tab| {
            tab.kind == SourceKind::Journal
                && tab.unit.trim() == unit.trim()
                && tab.running.is_some_and(|(_, running)| running == session)
        })
    }

    pub fn following_file(&self, path: &str, session: SessionId) -> Option<usize> {
        self.tabs.iter().position(|tab| {
            tab.kind == SourceKind::File
                && tab.path.trim() == path.trim()
                && tab.running.is_some_and(|(_, running)| running == session)
        })
    }

    pub fn trim_all(&mut self, max_lines: usize) {
        for tab in &mut self.tabs {
            tab.trim(max_lines);
        }
    }
}

#[cfg(test)]
mod tests;

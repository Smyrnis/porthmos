use porthmos_core::SessionId;

use crate::pane::Pane;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub preserves_times: bool,
    pub healthy: bool,
    pub shell_available: bool,
    pub pane: Pane,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabInfo {
    pub id: SessionId,
    pub name: String,
    pub healthy: bool,
    pub active: bool,
}

#[derive(Debug, Default)]
pub struct Sessions {
    items: Vec<Session>,
    active: Option<usize>,
}

impl Sessions {
    pub fn insert(&mut self, id: SessionId, name: String) {
        self.items.push(Session {
            id,
            name,
            preserves_times: false,
            healthy: true,
            shell_available: false,
            pane: Pane::awaiting_first_listing(),
        });
        self.active = Some(self.items.len() - 1);
    }

    pub fn remove(&mut self, id: SessionId) -> Option<Session> {
        let index = self.items.iter().position(|session| session.id == id)?;
        let removed = self.items.remove(index);
        self.active = match self.active {
            _ if self.items.is_empty() => None,
            Some(active) if active == index => Some(active.min(self.items.len() - 1)),
            Some(active) if active > index => Some(active - 1),
            other => other,
        };
        Some(removed)
    }

    pub fn set_shell_available(&mut self, id: SessionId, available: bool) {
        if let Some(session) = self.items.iter_mut().find(|session| session.id == id) {
            session.shell_available = available;
        }
    }

    pub fn active(&self) -> Option<&Session> {
        self.active.and_then(|index| self.items.get(index))
    }

    pub fn active_mut(&mut self) -> Option<&mut Session> {
        self.active.and_then(|index| self.items.get_mut(index))
    }

    pub fn set_active(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.active = Some(index);
        true
    }

    pub fn activate(&mut self, id: SessionId) -> bool {
        match self.items.iter().position(|session| session.id == id) {
            Some(index) => self.set_active(index),
            None => false,
        }
    }

    pub fn next(&mut self) {
        if let Some(active) = self.active {
            self.active = Some((active + 1) % self.items.len());
        }
    }

    pub fn previous(&mut self) {
        if let Some(active) = self.active {
            self.active = Some((active + self.items.len() - 1) % self.items.len());
        }
    }

    pub fn by_name(&self, name: &str) -> Option<&Session> {
        self.items.iter().find(|session| session.name == name)
    }

    #[cfg(test)]
    pub fn by_id(&self, id: SessionId) -> Option<&Session> {
        self.items.iter().find(|session| session.id == id)
    }

    pub fn by_id_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.items.iter_mut().find(|session| session.id == id)
    }

    pub fn names(&self) -> Vec<TabInfo> {
        self.items
            .iter()
            .enumerate()
            .map(|(index, session)| TabInfo {
                id: session.id,
                name: session.name.clone(),
                healthy: session.healthy,
                active: self.active == Some(index),
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests;

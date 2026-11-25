use porthmos_core::i18n::label as tr;
use porthmos_core::{Command, MonitorState, Nice, ProcessSignal, SessionId};

use super::App;
use crate::drawer::DrawerView;
use crate::health_view::{ActionState, HealthState, action_state};
use crate::modal::Modal;

impl App {
    pub fn health(&self) -> &HealthState {
        &self.health
    }

    pub fn health_tab_label(&self) -> String {
        tr("Health")
    }

    pub fn health_target(&self) -> Option<SessionId> {
        let chosen = self.health.chosen.and_then(|id| self.sessions.by_id(id)).filter(|session| session.healthy);
        chosen.or_else(|| self.sessions.active().filter(|session| session.healthy)).map(|session| session.id)
    }

    pub fn health_session_names(&self) -> Vec<String> {
        self.sessions.names().into_iter().filter(|tab| tab.healthy).map(|tab| tab.name).collect()
    }

    pub(in crate::app) fn show_health(&mut self) {
        self.dismiss_inline_editors();
        self.drawer.view = DrawerView::Health;
        self.drawer.open = true;
        self.drawer.focused = false;
    }

    pub(in crate::app) fn sync_monitor(&mut self) {
        let visible = self.drawer.open && self.drawer.view == DrawerView::Health;
        let wanted = if visible { self.health_target() } else { None };
        if wanted == self.health.started {
            return;
        }
        if let Some(old) = self.health.started.take() {
            self.core.send(Command::StopMonitor { session: old });
        }
        match wanted {
            Some(session) => {
                self.health.begin(session);
                self.core.send(Command::StartMonitor { session });
                self.health.started = Some(session);
            }
            None => self.health.clear(),
        }
    }

    pub(in crate::app) fn health_arrived(&mut self, session: SessionId, state: MonitorState) {
        self.health.apply(session, state);
    }

    pub(in crate::app) fn health_session_chosen(&mut self, name: String) {
        if let Some(session) = self.sessions.by_name(&name) {
            self.health.chosen = Some(session.id);
        }
    }

    pub(in crate::app) fn health_retry(&mut self) {
        let Some(session) = self.health.started else {
            return;
        };
        self.core.send(Command::StopMonitor { session });
        self.core.send(Command::StartMonitor { session });
        self.health.begin(session);
    }
}

impl App {
    fn health_selection(&self) -> Option<(SessionId, u32, String, i32)> {
        let session = self.health.started?;
        let snapshot = match &self.health.state {
            Some(MonitorState::Snapshot(snapshot)) => snapshot,
            _ => return None,
        };
        let pid = self.health.selected?;
        if action_state(snapshot, Some(pid)) != ActionState::Allowed {
            return None;
        }
        let process = snapshot.processes.iter().find(|process| process.pid == pid)?;
        Some((session, pid, process.name.clone(), process.nice))
    }

    pub(in crate::app) fn health_signal_requested(&mut self, signal: ProcessSignal) {
        if let Some((session, pid, name, _)) = self.health_selection() {
            self.modals.push(Modal::ConfirmSignal { session, pid, name, signal });
        }
    }

    pub(in crate::app) fn health_renice_requested(&mut self) {
        if let Some((session, pid, name, nice)) = self.health_selection() {
            self.modals.push(Modal::Renice { session, pid, name, text: nice.to_string(), error: None });
        }
    }

    pub(in crate::app) fn health_renice_changed(&mut self, typed: String) {
        if let Some(Modal::Renice { text, error, .. }) = self.modals.front_mut() {
            *text = typed;
            *error = None;
        }
    }

    pub(in crate::app) fn renice_rejected(&mut self) -> bool {
        let Some(Modal::Renice { text, error, .. }) = self.modals.front_mut() else {
            return false;
        };
        if parse_nice(text).is_some() {
            return false;
        }
        *error = Some(porthmos_core::i18n::text("gui.health.nice_invalid"));
        true
    }

    pub(in crate::app) fn send_renice(&mut self, session: SessionId, pid: u32, name: String, text: &str) {
        if let Some(nice) = parse_nice(text) {
            self.core.send(Command::ReniceProcess { session, pid, name, nice });
        }
    }
}

fn parse_nice(text: &str) -> Option<Nice> {
    text.trim().parse::<i32>().ok().and_then(Nice::new)
}

#[cfg(test)]
mod tests;

use porthmos_core::{Command, offer_summary};

use super::App;

pub fn offer_label(names: &[String]) -> String {
    format!("Reopen {}", offer_summary(names))
}

impl App {
    pub fn reopen_offer(&self) -> &[String] {
        &self.reopen_offer
    }

    pub(in crate::app) fn reopen_pressed(&mut self) {
        if self.reopen_offer.is_empty() {
            return;
        }
        self.reopen_offer.clear();
        self.core.send(Command::ReopenConnections);
    }

    pub(in crate::app) fn report_active_connection(&mut self) {
        let active = self.sessions.active().map(|session| session.id);
        if active != self.reported_connection {
            self.reported_connection = active;
            self.core.send(Command::ActiveConnection { session: active });
        }
    }

    pub(in crate::app) fn focus_connection(&mut self, name: &str) {
        if let Some(id) = self.sessions.by_name(name).map(|session| session.id) {
            self.sessions.activate(id);
        }
    }
}

#[cfg(test)]
mod tests;

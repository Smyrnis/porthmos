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
}

#[cfg(test)]
mod tests;

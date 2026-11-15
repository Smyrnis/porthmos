use porthmos_core::{Command, Severity, UpdateMode, UpdateState};

use super::App;
use crate::modal::Modal;
use crate::update_view::{Badge, UpdateView};

const UPDATE_TITLE: &str = "Update";

impl App {
    pub fn with_update_mode(mut self, mode: UpdateMode) -> App {
        self.update = UpdateView::new(mode);
        self
    }

    pub fn update_badge(&self) -> Option<Badge> {
        self.update.badge()
    }

    pub fn update_announcement(&self) -> Option<String> {
        self.update.announcement()
    }

    pub(in crate::app) fn update_arrived(&mut self, state: UpdateState) {
        if let Some(announcement) = self.update.apply(state) {
            self.push_notice(Severity::Info, announcement);
        }
    }

    pub(in crate::app) fn check_for_updates(&mut self) {
        match self.update.announcement() {
            Some(announcement) => self.push_notice(Severity::Info, announcement),
            None => self.core.send(Command::CheckForUpdate { manual: true }),
        }
    }

    pub(in crate::app) fn show_update_announcement(&mut self) {
        if let Some(body) = self.update.announcement() {
            self.modals.push(Modal::Info { title: UPDATE_TITLE.to_string(), body });
        }
    }
}

#[cfg(test)]
mod tests;

use iced::Task;
use porthmos_core::properties::{Digest, Measured};
use porthmos_core::{Command, VfsPath};

use super::{App, Message, PanelSide};
use crate::modal::properties::PropertiesPane;
use crate::modal::{Modal, properties_title};
use crate::panel::Panel;

impl App {
    pub(in crate::app) fn open_properties(&mut self, side: PanelSide) {
        let entries = self.panel_of(side).map(Panel::target_entries).unwrap_or_default();
        let Some(location) = self.location_of(side).filter(|_| !entries.is_empty()) else {
            return;
        };
        let paths = entries.iter().map(|entry| entry.path.clone()).collect();
        let files = entries.iter().filter(|entry| !entry.is_dir).map(|entry| (entry.path.clone(), entry.name.clone()));
        let pane = PropertiesPane::new(properties_title(&entries), files.collect());
        self.next_properties_id += 1;
        let request_id = self.next_properties_id;
        self.core.send(Command::Measure { request_id, location, paths });
        self.modals.push(Modal::Properties { location, request_id, pane });
    }

    pub(in crate::app) fn properties_escape(&mut self) -> bool {
        let Some(Modal::Properties { request_id, pane, .. }) = self.modals.front_mut() else {
            return false;
        };
        if !pane.running() {
            return false;
        }
        pane.cancel_running();
        let request_id = *request_id;
        self.core.send(Command::CancelProperties { request_id });
        true
    }

    pub(in crate::app) fn properties_compute(&mut self) {
        let Some(Modal::Properties { location, request_id, pane }) = self.modals.front_mut() else {
            return;
        };
        let (location, request_id) = (*location, *request_id);
        if let Some(paths) = pane.begin_checksums() {
            self.core.send(Command::Checksum { request_id, location, paths });
        }
    }

    pub(in crate::app) fn properties_expected_typed(&mut self, text: String) {
        if let Some(Modal::Properties { pane, .. }) = self.modals.front_mut() {
            pane.expected = text;
        }
    }

    pub(in crate::app) fn properties_copy_digest(&mut self, index: usize) -> Task<Message> {
        match self.modals.front() {
            Some(Modal::Properties { pane, .. }) => {
                pane.digest_text(index).map_or_else(Task::none, iced::clipboard::write)
            }
            _ => Task::none(),
        }
    }

    fn properties_pane_for(&mut self, request_id: u64) -> Option<&mut PropertiesPane> {
        self.modals.iter_mut().find_map(|modal| match modal {
            Modal::Properties { request_id: open, pane, .. } if *open == request_id => Some(pane),
            _ => None,
        })
    }

    pub(in crate::app) fn properties_progress(&mut self, request_id: u64, measured: Measured) {
        if let Some(pane) = self.properties_pane_for(request_id) {
            pane.set_progress(measured);
        }
    }

    pub(in crate::app) fn properties_measured(&mut self, request_id: u64, result: Result<Measured, String>) {
        if let Some(pane) = self.properties_pane_for(request_id) {
            pane.set_measured(result);
        }
    }

    pub(in crate::app) fn properties_checksum_progress(
        &mut self, request_id: u64, path: &VfsPath, done: u64, total: u64,
    ) {
        if let Some(pane) = self.properties_pane_for(request_id) {
            pane.set_checksum_progress(path, done, total);
        }
    }

    pub(in crate::app) fn properties_checksummed(
        &mut self, request_id: u64, path: &VfsPath, result: Result<Digest, String>,
    ) {
        if let Some(pane) = self.properties_pane_for(request_id) {
            pane.set_checksum(path, result);
        }
    }
}

#[cfg(test)]
mod tests;

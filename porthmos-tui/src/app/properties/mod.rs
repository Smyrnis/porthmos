use porthmos_core::i18n;
use porthmos_core::{
    Command, Message, RequestId, VfsPath,
    properties::{Digest, Measured},
};

use super::{App, PendingAction, Screen};
use crate::widgets::dialog::{
    Dialog,
    properties::{PropertiesChoice, PropertiesDialog},
};

impl App {
    pub(super) fn open_properties_dialog(&mut self) {
        if self.screen != Screen::Files {
            return;
        }
        let Some(location) = self.active_location() else {
            return;
        };
        let Some(panel) = self.panel(location) else {
            return;
        };
        let entries = panel.target_entries();
        if entries.is_empty() {
            return;
        }
        let paths: Vec<VfsPath> = entries.iter().map(|entry| entry.path.clone()).collect();
        let files = entries.iter().filter(|entry| !entry.is_dir).map(|entry| (entry.path.clone(), entry.name.clone()));
        let title = match entries.as_slice() {
            [only] => i18n::text_with("properties.title_one", &[("name", &only.name)]),
            many => i18n::text_with("properties.title_many", &[("count", &many.len().to_string())]),
        };
        let dialog = PropertiesDialog::new(title, files.collect());
        self.next_properties_id += 1;
        let request_id = self.next_properties_id;
        self.core.send(Command::Measure { request_id, location, paths });
        self.dialog = Some(Dialog::Properties(dialog));
        self.pending_action = Some(PendingAction::Properties { request_id, location });
    }

    pub(super) fn apply_properties_choice(&mut self, choice: PropertiesChoice) {
        let Some(&PendingAction::Properties { request_id, location }) = self.pending_action.as_ref() else {
            return;
        };
        match choice {
            PropertiesChoice::Pending => {}
            PropertiesChoice::Compute(paths) => self.core.send(Command::Checksum { request_id, location, paths }),
            PropertiesChoice::CancelRunning => self.core.send(Command::CancelProperties { request_id }),
            PropertiesChoice::Close => {
                self.dialog = None;
                self.pending_action = None;
                self.core.send(Command::CancelProperties { request_id });
            }
        }
    }

    fn properties_dialog_for(&mut self, request_id: RequestId) -> Option<&mut PropertiesDialog> {
        match (&self.pending_action, self.dialog.as_mut()) {
            (Some(PendingAction::Properties { request_id: open, .. }), Some(Dialog::Properties(dialog)))
                if *open == request_id =>
            {
                Some(dialog)
            }
            _ => None,
        }
    }

    pub(super) fn apply_measure_progress(&mut self, request_id: RequestId, measured: Measured) {
        if let Some(dialog) = self.properties_dialog_for(request_id) {
            dialog.set_progress(measured);
        }
    }

    pub(super) fn apply_measured(&mut self, request_id: RequestId, result: Result<Measured, Message>) {
        if let Some(dialog) = self.properties_dialog_for(request_id) {
            dialog.set_measured(result);
        }
    }

    pub(super) fn apply_checksum_progress(&mut self, request_id: RequestId, path: &VfsPath, done: u64, total: u64) {
        if let Some(dialog) = self.properties_dialog_for(request_id) {
            dialog.set_checksum_progress(path, done, total);
        }
    }

    pub(super) fn apply_checksummed(&mut self, request_id: RequestId, path: &VfsPath, result: Result<Digest, Message>) {
        if let Some(dialog) = self.properties_dialog_for(request_id) {
            dialog.set_checksum(path, result);
        }
    }
}

#[cfg(test)]
mod tests;

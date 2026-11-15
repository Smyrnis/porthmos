use porthmos_core::{
    Severity,
    folder_compare::{FolderMarks, compare_folders, needs_server_message},
};

use super::{App, Screen};

impl App {
    pub(super) fn toggle_compare_folders(&mut self) {
        if self.screen != Screen::Files {
            return;
        }
        if self.compare_folders {
            self.compare_folders = false;
            return;
        }
        let Some(marks) = self.folder_marks() else {
            self.notifications.push(Severity::Warning, needs_server_message());
            return;
        };
        self.compare_folders = true;
        self.notifications.push(Severity::Info, marks.summary());
    }

    pub(super) fn visible_folder_marks(&self) -> Option<FolderMarks> {
        if self.compare_folders { self.folder_marks() } else { None }
    }

    fn folder_marks(&self) -> Option<FolderMarks> {
        let session = self.sessions.active()?;
        Some(compare_folders(self.local.entries(), session.panel.entries()))
    }
}

#[cfg(test)]
mod tests;

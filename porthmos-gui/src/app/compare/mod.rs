use porthmos_core::Severity;
use porthmos_core::folder_compare::{FolderMarks, compare_folders, needs_server_message};

use super::App;

impl App {
    pub fn comparing_folders(&self) -> bool {
        self.compare_folders
    }

    pub fn compare_possible(&self) -> bool {
        self.sessions.active().is_some()
    }

    pub fn folder_marks(&self) -> Option<FolderMarks> {
        if !self.comparing_folders() {
            return None;
        }
        self.current_folder_marks()
    }

    fn current_folder_marks(&self) -> Option<FolderMarks> {
        let session = self.sessions.active()?;
        Some(compare_folders(self.local.panel.entries(), session.pane.panel.entries()))
    }

    pub(in crate::app) fn toggle_compare_folders(&mut self) {
        if self.compare_folders {
            self.compare_folders = false;
            return;
        }
        let Some(marks) = self.current_folder_marks() else {
            self.push_notice(Severity::Warning, needs_server_message());
            return;
        };
        self.compare_folders = true;
        self.push_notice(Severity::Info, marks.summary());
    }
}

#[cfg(test)]
mod tests;

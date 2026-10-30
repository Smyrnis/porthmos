use std::path::PathBuf;

use porthmos_core::edit::{EditChoice, EditQuestionKind, EditorCommand, EditorExit};
use porthmos_core::{Command, Severity};

use super::App;
use crate::editing::CommandSpec;
use crate::editing::{EditorKind, Platform, classify, default_app_spec, editor_spec};
use crate::modal::Modal;
use crate::processes::{ProcessEvent, ProcessJob};

#[derive(Debug, Clone, Copy)]
pub enum JobRole {
    Edit { edit_id: u64, default_app: bool },
    Terminal,
}

impl App {
    #[cfg(test)]
    pub fn edits_busy(&self) -> bool {
        self.edits_busy
    }

    #[cfg(test)]
    pub fn set_launcher(&mut self, launcher: crate::processes::Launcher) {
        self.launcher = launcher;
    }

    pub fn set_state_dir(&mut self, state_dir: PathBuf) {
        self.launcher.set_state_dir(state_dir);
    }

    pub(in crate::app) fn start_edit(&mut self) {
        let Some(panel) = self.panel_of(self.active) else {
            return;
        };
        let file = match panel.target_entries().as_slice() {
            [entry] if !entry.is_dir => entry.clone(),
            [] | [_] => return self.push_notice(Severity::Warning, "Can't edit a folder".to_string()),
            _ => return self.push_notice(Severity::Warning, "Select one file to edit".to_string()),
        };
        if let Some(location) = self.location_of(self.active) {
            self.core.send(Command::EditFile { location, path: file.path });
        }
    }

    fn spawn_edit(
        &mut self, edit_id: u64, spec: CommandSpec, env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
        cleanup: Vec<PathBuf>, default_app: bool,
    ) {
        let token = self.next_process_token;
        self.next_process_token += 1;
        self.jobs.insert(token, JobRole::Edit { edit_id, default_app });
        self.launcher.spawn(ProcessJob { token, spec, env, cleanup });
    }

    fn finish_edit(&mut self, edit_id: u64, exit: EditorExit) {
        self.core.send(Command::FinishEdit { edit_id, exit });
    }

    pub(in crate::app) fn edit_ready(&mut self, edit_id: u64, file: PathBuf, editor: EditorCommand) {
        let platform: Platform = self.launcher.platform();
        match classify(&editor, platform) {
            EditorKind::Window => self.spawn_edit(edit_id, editor_spec(&editor, &file), Vec::new(), Vec::new(), false),
            EditorKind::Terminal => match self.launcher.terminal(editor_spec(&editor, &file), Vec::new(), true) {
                Ok(launch) => self.spawn_edit(edit_id, launch.spec, launch.env, launch.cleanup, false),
                Err(error) => self.finish_edit(edit_id, EditorExit::LaunchFailed(error.to_string())),
            },
            EditorKind::DefaultApp => {
                let name = file.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
                self.spawn_edit(edit_id, default_app_spec(&file, platform), Vec::new(), Vec::new(), true);
                self.modals.push(Modal::EditWaiting { edit_id, name });
            }
        }
    }

    pub(in crate::app) fn process_finished(&mut self, event: ProcessEvent) {
        let ProcessEvent::Exited { token, exit } = event;
        let Some(role) = self.jobs.remove(&token) else {
            return;
        };
        let (edit_id, default_app) = match role {
            JobRole::Edit { edit_id, default_app } => (edit_id, default_app),
            JobRole::Terminal => return self.terminal_finished(exit),
        };
        if default_app {
            if exit == EditorExit::Success {
                return;
            }
            self.modals.remove_where(
                |modal| matches!(modal, Modal::EditWaiting { edit_id: waiting, .. } if *waiting == edit_id),
            );
        }
        self.finish_edit(edit_id, exit);
    }

    pub(in crate::app) fn edit_question(&mut self, edit_id: u64, name: String, kind: EditQuestionKind) {
        let queued =
            self.modals.any(|modal| matches!(modal, Modal::EditQuestion { edit_id: queued, .. } if *queued == edit_id));
        if !queued {
            self.modals.push(Modal::EditQuestion { edit_id, name, kind });
        }
    }

    pub(in crate::app) fn resolve_edit_question(&mut self, accepted: bool, edit_id: u64, kind: EditQuestionKind) {
        let choice = match (accepted, kind) {
            (true, EditQuestionKind::Upload) => EditChoice::Upload,
            (true, EditQuestionKind::Conflict) => EditChoice::KeepCopy,
            (false, _) => EditChoice::Cancel,
        };
        self.core.send(Command::ResolveEdit { edit_id, choice });
    }

    pub(in crate::app) fn edit_answered(&mut self, choice: EditChoice) {
        if !matches!(self.modals.front(), Some(Modal::EditQuestion { .. })) {
            return;
        }
        if let Some(Modal::EditQuestion { edit_id, .. }) = self.modals.pop() {
            self.core.send(Command::ResolveEdit { edit_id, choice });
        }
    }

    pub(in crate::app) fn finish_waiting_edit(&mut self, edit_id: u64) {
        self.jobs.retain(|_, role| !matches!(role, JobRole::Edit { edit_id: owner, .. } if *owner == edit_id));
        self.finish_edit(edit_id, EditorExit::Success);
    }
}

#[cfg(test)]
mod tests;

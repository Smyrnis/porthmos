use porthmos_core::{
    Answer,
    delete_prompt::{
        THIS_COMPUTER,
        count::{Contents, contents_line},
        delete_summary,
    },
    names::entry_name_problem,
    permissions::{mode_text, parse_mode},
};

use super::*;
use crate::widgets::history_view::printable;

fn prompt_text(summary: &[String], contents: Option<Contents>) -> String {
    let mut lines = summary.to_vec();
    lines.extend(contents.as_ref().map(contents_line));
    lines.join("\n")
}

impl App {
    pub(super) fn open_mkdir_dialog(&mut self) {
        if self.screen != Screen::Files {
            return;
        }
        if self.active_panel == ActivePanel::Remote && self.sessions.active().is_none() {
            return;
        }

        self.dialog = Some(Dialog::TextInput(TextInputDialog::new("New directory name", "")));
        self.pending_action = Some(PendingAction::Mkdir);
    }

    pub(super) fn open_rename_dialog(&mut self) {
        if self.screen != Screen::Files {
            return;
        }

        let Some(location) = self.active_location() else {
            return;
        };
        let Some(entry) = self.panel(location).and_then(|panel| panel.current_entry()) else {
            return;
        };
        let (from, current_name) = (entry.path.clone(), entry.name.clone());

        self.dialog = Some(Dialog::TextInput(TextInputDialog::new("Rename to", current_name)));
        self.pending_action = Some(PendingAction::Rename { location, from });
    }

    pub(super) fn open_permissions_dialog(&mut self) {
        if self.screen != Screen::Files {
            return;
        }

        let Some(location) = self.active_location() else {
            return;
        };
        let Some(panel) = self.panel(location) else {
            return;
        };
        let (entries, paths) = (panel.target_entries(), panel.targets());
        if paths.is_empty() {
            return;
        }
        let current = entries.iter().find_map(|entry| entry.permissions).map(mode_text).unwrap_or_default();
        let title = match paths.len() {
            1 => "Permissions (octal)".to_string(),
            count => format!("Permissions of {count} items (octal)"),
        };

        self.dialog = Some(Dialog::TextInput(TextInputDialog::new(title, current)));
        self.pending_action = Some(PendingAction::Permissions { location, paths });
    }

    pub(super) fn open_delete_dialog(&mut self) {
        if self.screen != Screen::Files {
            return;
        }

        let Some(location) = self.active_location() else {
            return;
        };
        let Some(panel) = self.panel(location) else {
            return;
        };
        let (entries, targets) = (panel.target_entries(), panel.targets());
        if targets.is_empty() {
            return;
        }

        let place = match location {
            Location::Local => THIS_COMPUTER.to_string(),
            Location::Session(id) => self.sessions.by_id(id).map(|session| session.name.clone()).unwrap_or_default(),
        };
        let summary = delete_summary(&entries, &place, printable);
        let folders: Vec<VfsPath> =
            entries.iter().filter(|entry| entry.is_dir).map(|entry| entry.path.clone()).collect();
        let count = if folders.is_empty() {
            None
        } else {
            self.next_count_id += 1;
            let request_id = self.next_count_id;
            self.core.send(Command::CountContents { request_id, location, folders });
            Some(DeleteCount { request_id, summary: summary.clone(), answered: false })
        };
        let message = prompt_text(&summary, count.as_ref().map(|_| Contents::Counting));

        self.dialog = Some(Dialog::Confirm(ConfirmDialog::new(message)));
        self.pending_action = Some(PendingAction::Delete { location, paths: targets, count });
    }

    fn stop_counting(&self, count: Option<DeleteCount>) {
        if count.is_some_and(|count| !count.answered) {
            self.core.send(Command::CancelCount);
        }
    }

    pub(super) fn apply_contents_counted(&mut self, request_id: RequestId, contents: Contents) {
        let Some(PendingAction::Delete { count: Some(pending), .. }) = self.pending_action.as_mut() else {
            return;
        };
        if pending.request_id != request_id || pending.answered {
            return;
        }
        pending.answered = true;
        let text = prompt_text(&pending.summary, Some(contents));
        if let Some(Dialog::Confirm(dialog)) = self.dialog.as_mut() {
            dialog.message = text;
        }
    }

    fn create_directory(&mut self, name: &str) {
        let Some(location) = self.active_location() else {
            return;
        };
        let Some(panel) = self.panel(location) else {
            return;
        };
        let path = panel.path().join(name);
        self.core.send(Command::CreateDir { location, path });
    }

    fn rename_entry(&mut self, location: Location, from: VfsPath, new_name: &str) {
        let Some(to) = from.parent().map(|parent| parent.join(new_name)) else {
            return;
        };
        self.core.send(Command::Rename { location, from, to });
    }

    fn submitted_mode_problem(&self, text: &str) -> Option<String> {
        match self.pending_action.as_ref()? {
            PendingAction::Permissions { .. } => parse_mode(text).err(),
            _ => None,
        }
    }

    fn submitted_name_problem(&self, name: &str) -> Option<String> {
        let (location, original) = match self.pending_action.as_ref()? {
            PendingAction::Mkdir => (self.active_location()?, None),
            PendingAction::Rename { location, from } => (*location, from.file_name().and_then(|name| name.to_str())),
            _ => return None,
        };
        let panel = self.panel(location);
        entry_name_problem(name, original, |candidate| panel.is_some_and(|panel| panel.has_entry_named(candidate)))
    }

    pub(super) fn apply_dialog_key(&mut self, key: KeyEvent) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };

        match dialog.handle_key(key) {
            DialogOutcome::Pending => {}
            DialogOutcome::Cancelled => {
                self.dialog = None;
                self.reveal = None;
                match self.pending_action.take() {
                    Some(
                        PendingAction::SubmitPassword { request_id }
                        | PendingAction::TrustHostKey { request_id }
                        | PendingAction::TrustCertificate { request_id },
                    ) => {
                        self.core.send(Command::Answer { request_id, answer: None, save: false });
                        self.connection_status = ConnectionStatus::Disconnected;
                    }
                    Some(PendingAction::SubmitTestPassword { request_id }) => {
                        self.core.send(Command::Answer { request_id, answer: None, save: false });
                    }
                    Some(PendingAction::EditUpload { edit_id } | PendingAction::EditConflict { edit_id }) => {
                        self.core.send(Command::ResolveEdit { edit_id, choice: EditChoice::Cancel });
                    }
                    Some(PendingAction::Delete { count, .. }) => self.stop_counting(count),
                    _ => {}
                }
            }
            DialogOutcome::Confirmed => {
                self.dialog = None;
                match self.pending_action.take() {
                    Some(PendingAction::Delete { location, paths, count }) => {
                        self.stop_counting(count);
                        self.core.send(Command::Delete { location, paths })
                    }
                    Some(PendingAction::DeleteConnection { name }) => {
                        self.core.send(Command::DeleteProfile { name });
                    }
                    Some(PendingAction::ClearHistory) => self.core.send(Command::ClearHistory),
                    Some(PendingAction::ConfirmQuit) => self.should_quit = true,
                    Some(PendingAction::ConfirmSyncDelete { sync_id, choices }) => self.send_sync_run(sync_id, choices),
                    Some(PendingAction::EditUpload { edit_id }) => {
                        self.core.send(Command::ResolveEdit { edit_id, choice: EditChoice::Upload })
                    }
                    Some(
                        PendingAction::TrustHostKey { request_id } | PendingAction::TrustCertificate { request_id },
                    ) => {
                        self.core.send(Command::Answer { request_id, answer: Some(Answer::Confirmed), save: false });
                    }
                    _ => {}
                }
            }
            DialogOutcome::Submitted(value)
                if self.submitted_name_problem(&value).or_else(|| self.submitted_mode_problem(&value)).is_some() =>
            {
                let problem = self.submitted_name_problem(&value).or_else(|| self.submitted_mode_problem(&value));
                if let Some(Dialog::TextInput(input)) = self.dialog.as_mut() {
                    input.error = problem;
                }
            }
            DialogOutcome::Submitted(value) => {
                self.dialog = None;
                match self.pending_action.take() {
                    Some(PendingAction::Mkdir) => self.create_directory(&value),
                    Some(PendingAction::Rename { location, from }) => self.rename_entry(location, from, &value),
                    Some(PendingAction::Permissions { location, paths }) => {
                        if let Ok(mode) = parse_mode(&value) {
                            self.core.send(Command::SetPermissions { location, paths, mode });
                        }
                    }
                    Some(PendingAction::AddBookmark) => self.add_bookmark(value),
                    Some(PendingAction::SpeedLimit) => self.apply_speed_limit(&value),
                    Some(PendingAction::SubmitPassword { .. })
                    | Some(PendingAction::SubmitTestPassword { .. })
                    | Some(PendingAction::Delete { .. })
                    | Some(PendingAction::TrustHostKey { .. })
                    | Some(PendingAction::TrustCertificate { .. })
                    | Some(PendingAction::AddConnection)
                    | Some(PendingAction::EditConnection { .. })
                    | Some(PendingAction::DeleteConnection { .. })
                    | Some(PendingAction::EditSshLabels { .. })
                    | Some(PendingAction::FixMissingHost { .. })
                    | Some(PendingAction::MoveLabels { .. })
                    | Some(PendingAction::ResolveConflict)
                    | Some(PendingAction::ClearHistory)
                    | Some(PendingAction::EditUpload { .. })
                    | Some(PendingAction::EditConflict { .. })
                    | Some(PendingAction::ConfirmQuit)
                    | Some(PendingAction::StartSync { .. })
                    | Some(PendingAction::ConfirmSyncDelete { .. })
                    | None => {}
                }
            }
            DialogOutcome::Selected(index) => {
                self.dialog = None;
                match self.pending_action.take() {
                    Some(PendingAction::FixMissingHost { name }) => self.apply_missing_host_choice(name, index),
                    Some(PendingAction::EditConflict { edit_id }) => self.answer_edit_conflict(edit_id, index),
                    Some(PendingAction::MoveLabels { from, candidates }) => {
                        self.move_labels_to(from, candidates, index)
                    }
                    other => {
                        self.pending_action = other;
                        self.navigate_to_bookmark(index);
                    }
                }
            }
            DialogOutcome::Removed(index) => {
                self.core.send(Command::RemoveBookmark { index });
                if let Some(Dialog::List(list)) = self.dialog.as_mut() {
                    list.items.remove(index);
                    if list.cursor >= list.items.len() {
                        list.cursor = list.items.len().saturating_sub(1);
                    }
                }
            }
            DialogOutcome::FormSubmitted(values) => match &self.pending_action {
                Some(PendingAction::AddConnection) => self.submit_connection_form(values, None),
                Some(PendingAction::EditConnection { original }) => {
                    let original = original.name.clone();
                    self.submit_connection_form(values, Some(original));
                }
                Some(PendingAction::EditSshLabels { name }) => {
                    let name = name.clone();
                    self.submit_ssh_labels(name, values);
                }
                Some(PendingAction::SubmitPassword { request_id }) => {
                    let request_id = *request_id;
                    self.submit_password(request_id, values);
                }
                Some(PendingAction::SubmitTestPassword { request_id }) => {
                    let request_id = *request_id;
                    self.submit_test_password(request_id, values);
                }
                Some(PendingAction::StartSync { session }) => {
                    let session = *session;
                    self.submit_sync_options(session, values);
                }
                _ => self.dialog = None,
            },
            DialogOutcome::FormTest(values) => self.start_connection_test(values),
            DialogOutcome::FormChoiceChanged { key } => self.form_choice_changed(key),
            DialogOutcome::Resolved { resolution, apply_to_rest } => {
                self.dialog = None;
                self.pending_action = None;
                self.answer_conflict(resolution, apply_to_rest);
            }
        }
        self.restore_stashed_form();
        self.sync_connection_test();
        self.open_next_conflict_prompt();
        self.announce_missing_hosts();
    }
}

#[cfg(test)]
mod tests;

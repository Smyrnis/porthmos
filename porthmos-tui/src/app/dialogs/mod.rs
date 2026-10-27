use porthmos_core::Answer;

use super::*;

fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("A name can't be empty")
    } else if name == "." || name == ".." {
        Some("A name can't be \".\" or \"..\"")
    } else if name.contains('/') {
        Some("A name can't contain \"/\"")
    } else {
        None
    }
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

    pub(super) fn open_delete_dialog(&mut self) {
        if self.screen != Screen::Files {
            return;
        }

        let Some(location) = self.active_location() else {
            return;
        };
        let Some(targets) = self.panel(location).map(PanelView::targets) else {
            return;
        };
        if targets.is_empty() {
            return;
        }

        let message = if targets.len() == 1 {
            let name = targets[0].file_name().and_then(|name| name.to_str()).unwrap_or("?");
            format!("Delete \"{name}\"?")
        } else {
            format!("Delete {} selected items?", targets.len())
        };

        self.dialog = Some(Dialog::Confirm(ConfirmDialog::new(message)));
        self.pending_action = Some(PendingAction::Delete { location, paths: targets });
    }

    fn create_directory(&mut self, name: &str) {
        if self.refuse_bad_name(name) {
            return;
        }
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
        if self.refuse_bad_name(new_name) {
            return;
        }
        let Some(to) = from.parent().map(|parent| parent.join(new_name)) else {
            return;
        };
        self.core.send(Command::Rename { location, from, to });
    }

    fn refuse_bad_name(&mut self, name: &str) -> bool {
        let Some(problem) = name_problem(name) else {
            return false;
        };
        self.notifications.push(Severity::Warning, problem);
        true
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
                    _ => {}
                }
            }
            DialogOutcome::Confirmed => {
                self.dialog = None;
                match self.pending_action.take() {
                    Some(PendingAction::Delete { location, paths }) => {
                        self.core.send(Command::Delete { location, paths })
                    }
                    Some(PendingAction::DeleteConnection { name }) => {
                        self.core.send(Command::DeleteProfile { name });
                    }
                    Some(PendingAction::ClearHistory) => self.core.send(Command::ClearHistory),
                    Some(PendingAction::ConfirmQuit) => self.should_quit = true,
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
            DialogOutcome::Submitted(value) => {
                self.dialog = None;
                match self.pending_action.take() {
                    Some(PendingAction::Mkdir) => self.create_directory(&value),
                    Some(PendingAction::Rename { location, from }) => self.rename_entry(location, from, &value),
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

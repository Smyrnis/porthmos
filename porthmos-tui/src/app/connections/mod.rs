mod form;
mod missing;

use porthmos_core::{Answer, TestOutcome};

use super::*;
use crate::widgets::dialog::{FormDialog, FormField, FormLayout, TestState};

const NO_KEYRING_HINT: &str = "Passwords are not saved: no system keyring. They are kept until Porthmos quits.";

fn ssh_launch_failure(error: &std::io::Error, windows: bool) -> String {
    if windows && error.kind() == std::io::ErrorKind::NotFound {
        "ssh.exe was not found. Install the OpenSSH Client under Settings > Apps > Optional features, then try again."
            .to_string()
    } else {
        format!("Failed to launch ssh: {error}")
    }
}

fn ssh_exit_failure(code: Option<i32>, via_sshpass: bool) -> String {
    let Some(code) = code else {
        return "ssh was stopped before it finished".to_string();
    };
    let hint = match (via_sshpass, code) {
        (true, 5) => Some("the password is incorrect"),
        (true, 6) => Some("the host key is not known to ssh; add it to ~/.ssh/known_hosts"),
        (true, 7) => Some("the host key has changed; check ~/.ssh/known_hosts"),
        _ => None,
    };
    match hint {
        Some(hint) => format!("ssh exited with code {code} \u{2014} {hint}"),
        None => format!("ssh exited with code {code}"),
    }
}

impl App {
    pub(super) fn request_shell(&mut self) {
        let Some(session) = self.sessions.active() else {
            self.notifications.push(Severity::Warning, "Connect to a server first");
            return;
        };
        if !session.shell_available {
            self.notifications.push(Severity::Warning, "This connection doesn't support a terminal session");
            return;
        }
        let directory = Some(session.panel.path().clone());
        self.core.send(Command::PrepareShell { session: session.id, directory });
    }

    pub(super) async fn launch_shell(
        &mut self, terminal: &mut ratatui::Terminal<Backend>, invocation: ShellInvocation,
    ) -> Result<()> {
        terminal::restore()?;

        let via_sshpass = std::path::Path::new(&invocation.program).file_stem().is_some_and(|name| name == "sshpass");
        let ssh_result = tokio::task::spawn_blocking(move || invocation.to_command().status()).await;

        *terminal = terminal::init()?;

        match ssh_result {
            Ok(Ok(status)) if !status.success() => {
                self.error(ssh_exit_failure(status.code(), via_sshpass));
            }
            Ok(Ok(_)) => {}
            Ok(Err(io_err)) => {
                self.error(ssh_launch_failure(&io_err, cfg!(windows)));
            }
            Err(join_err) => {
                self.error(format!("ssh task failed: {join_err}"));
            }
        }

        Ok(())
    }

    pub(super) fn open_connections_screen(&mut self) {
        self.screen = Screen::Connections;
        self.core.send(Command::ListProfiles);
    }

    pub(super) fn details_pane_visible(&self) -> bool {
        layout::connections_layout(Rect::new(0, 0, self.body_width.get(), 1)).details.is_some()
    }

    pub(super) fn details_popup_visible(&self) -> bool {
        self.details_popup
            && self.screen == Screen::Connections
            && self.dialog.is_none()
            && self.connections.selected().is_some()
    }

    pub(super) fn open_details_popup(&mut self) {
        if !self.details_pane_visible() && self.connections.selected().is_some() {
            self.details_popup = true;
        }
    }

    pub(super) fn open_add_connection_dialog(&mut self) {
        if self.screen != Screen::Connections {
            return;
        }

        let Some(mut dialog) = form::build("Add connection", self.core.protocols(), None) else {
            self.notifications.push(Severity::Warning, "No protocols are available in this build");
            return;
        };
        dialog.hint = self.keyring_hint();
        self.dialog = Some(Dialog::Form(dialog));
        self.pending_action = Some(PendingAction::AddConnection);
    }

    pub(super) fn form_choice_changed(&mut self, key: &'static str) {
        if key != "protocol" {
            return;
        }
        let core = self.core.clone();
        if let Some(Dialog::Form(dialog)) = self.dialog.as_mut() {
            form::rebuild_for_protocol(dialog, core.protocols());
        }
    }

    pub(super) fn start_connection_test(&mut self, values: Vec<(&'static str, String)>) {
        let original = match &self.pending_action {
            Some(PendingAction::AddConnection) => None,
            Some(PendingAction::EditConnection { original }) => Some(original.name.clone()),
            _ => {
                if let Some(Dialog::Form(form)) = self.dialog.as_mut() {
                    form.test = None;
                }
                return;
            }
        };
        let protocol = values.iter().find(|(key, _)| *key == "protocol").map(|(_, value)| value.as_str()).unwrap_or("");
        let secret_keys = form::secret_keys(self.core.protocols(), protocol);
        let draft = form::draft(values, &secret_keys);
        let test_id = self.next_test_id;
        self.next_test_id += 1;
        self.connection_test = Some(test_id);
        self.core.send(Command::TestConnection { test_id, original, draft: Box::new(draft) });
    }

    pub(super) fn apply_test_finished(&mut self, test_id: u64, outcome: TestOutcome) {
        if self.connection_test != Some(test_id) {
            return;
        }
        self.connection_test = None;
        if let Some(form) = self.running_test_form() {
            form.test = Some(TestState::Finished(outcome));
        }
    }

    pub(super) fn sync_connection_test(&mut self) {
        if self.connection_test.is_some() && self.running_test_form().is_none() {
            self.cancel_connection_test();
        }
    }

    pub(super) fn cancel_connection_test(&mut self) {
        if self.connection_test.take().is_some() {
            self.core.send(Command::CancelTest);
        }
        if let Some(form) = self.running_test_form() {
            form.test = None;
        }
    }

    pub(super) fn submit_connection_form(&mut self, values: Vec<(&'static str, String)>, original: Option<String>) {
        self.cancel_connection_test();
        self.reveal = values
            .iter()
            .find(|(key, _)| *key == "name")
            .map(|(_, name)| Reveal::AwaitingSave(name.trim().to_string()));
        let protocol = values.iter().find(|(key, _)| *key == "protocol").map(|(_, value)| value.as_str()).unwrap_or("");
        let secret_keys = form::secret_keys(self.core.protocols(), protocol);
        self.core.send(Command::SaveProfile { original, draft: Box::new(form::draft(values, &secret_keys)) });
    }

    pub(super) fn open_edit_connection_dialog(&mut self) {
        if self.screen != Screen::Connections {
            return;
        }
        let Some(entry) = self.connections.selected_entry().cloned() else {
            return;
        };
        if entry.source.is_orphan_labels() {
            return;
        }
        if entry.source == ConnectionSource::SshConfig {
            let name = entry.name.clone();
            self.dialog = Some(Dialog::Form(form::build_labels(&entry)));
            self.pending_action = Some(PendingAction::EditSshLabels { name });
            return;
        }

        let Some(mut dialog) = form::build("Edit connection", self.core.protocols(), Some(&entry)) else {
            return;
        };
        dialog.hint = self.keyring_hint();
        self.dialog = Some(Dialog::Form(dialog));
        self.pending_action = Some(PendingAction::EditConnection { original: entry });
    }

    pub(super) fn open_delete_connection_dialog(&mut self) {
        if self.screen != Screen::Connections {
            return;
        }
        let Some(entry) = self.connections.selected_entry() else {
            return;
        };
        if entry.source.is_orphan_labels() {
            return;
        }
        if entry.source == ConnectionSource::SshConfig {
            self.notifications
                .push(Severity::Info, "This connection is defined in ~/.ssh/config and can't be deleted here");
            return;
        }

        let name = entry.name.clone();
        self.dialog = Some(Dialog::Confirm(ConfirmDialog::new(format!("Delete connection \"{name}\"?"))));
        self.pending_action = Some(PendingAction::DeleteConnection { name });
    }

    pub(super) fn submit_ssh_labels(&mut self, name: String, values: Vec<(&'static str, String)>) {
        let value = |wanted: &str| {
            values.iter().find(|(key, _)| *key == wanted).map(|(_, value)| value.clone()).unwrap_or_default()
        };
        let (group, tags, forget) = (value("group"), value("tags"), value("saved_password") == "forget");
        self.reveal = Some(Reveal::AwaitingSave(name.clone()));
        self.core.send(Command::SaveSshLabels { name: name.clone(), group, tags });
        if forget {
            self.core.send(Command::ForgetSshPassword { alias: name });
        }
    }

    fn keyring_hint(&self) -> Option<String> {
        (!self.keyring_available).then(|| NO_KEYRING_HINT.to_string())
    }

    pub(super) fn open_password_prompt(&mut self, request_id: RequestId, username: String, name: String) {
        let mut fields = vec![FormField::masked("password", "Password", "")];
        if self.keyring_available {
            let choices = vec![("true".to_string(), "Yes".to_string()), ("false".to_string(), "No".to_string())];
            let current = if self.save_choice { "true" } else { "false" };
            fields.push(FormField::choice("save", "Save in keyring", choices, current));
        }
        self.dialog = Some(Dialog::Form(FormDialog::new(format!("Password for {username}@{name}"), fields)));
        self.pending_action = Some(PendingAction::SubmitPassword { request_id });
    }

    pub(super) fn open_test_password_prompt(&mut self, request_id: RequestId, username: String, name: String) {
        let fields = vec![FormField::masked("password", "Password", "")];
        self.dialog = Some(Dialog::Form(FormDialog::new(format!("Password for {username}@{name}"), fields)));
        self.pending_action = Some(PendingAction::SubmitTestPassword { request_id });
    }

    pub(super) fn ask_test(&mut self, test_id: u64, request_id: RequestId, question: Question) {
        if self.connection_test != Some(test_id) {
            return;
        }
        match question {
            Question::Password { username, name } => {
                self.make_room_for_question();
                self.open_test_password_prompt(request_id, username, name);
            }
            Question::TrustHostKey { .. } | Question::TrustCertificate { .. } => {
                self.core.send(Command::Answer { request_id, answer: None, save: false });
            }
        }
    }

    pub(super) fn make_room_for_question(&mut self) {
        self.requeue_open_edit_question();
        self.answer_displaced_question();
        self.stash_screen_form();
    }

    fn answer_displaced_question(&mut self) {
        let request_id = match &self.pending_action {
            Some(
                PendingAction::SubmitPassword { request_id }
                | PendingAction::SubmitTestPassword { request_id }
                | PendingAction::TrustHostKey { request_id }
                | PendingAction::TrustCertificate { request_id },
            ) => *request_id,
            _ => return,
        };
        self.pending_action = None;
        self.core.send(Command::Answer { request_id, answer: None, save: false });
    }

    fn stash_screen_form(&mut self) {
        let is_screen_form = matches!(&self.dialog, Some(Dialog::Form(form)) if form.layout == FormLayout::Screen);
        if !is_screen_form || self.pending_action.is_none() {
            return;
        }
        if let (Some(dialog), Some(action)) = (self.dialog.take(), self.pending_action.take()) {
            self.stashed_form = Some((dialog, action));
        }
    }

    pub(super) fn restore_stashed_form(&mut self) {
        if self.dialog.is_some() {
            return;
        }
        if let Some((dialog, action)) = self.stashed_form.take() {
            self.dialog = Some(dialog);
            self.pending_action = Some(action);
        }
    }

    pub(super) fn submit_test_password(&mut self, request_id: RequestId, values: Vec<(&'static str, String)>) {
        let password =
            values.into_iter().find(|(key, _)| *key == "password").map(|(_, value)| value).unwrap_or_default();
        self.core.send(Command::Answer { request_id, answer: Some(Answer::Password(password)), save: false });
        self.dialog = None;
        self.pending_action = None;
    }

    fn running_test_form(&mut self) -> Option<&mut FormDialog> {
        let in_dialog = matches!(&self.dialog, Some(Dialog::Form(form)) if form.test == Some(TestState::Running));
        let candidate =
            if in_dialog { self.dialog.as_mut() } else { self.stashed_form.as_mut().map(|(dialog, _)| dialog) };
        match candidate {
            Some(Dialog::Form(form)) if form.test == Some(TestState::Running) => Some(form),
            _ => None,
        }
    }

    pub(super) fn submit_password(&mut self, request_id: RequestId, values: Vec<(&'static str, String)>) {
        let value = |wanted: &str| values.iter().find(|(key, _)| *key == wanted).map(|(_, value)| value.clone());
        let save = value("save").as_deref() == Some("true");
        if value("save").is_some() && save != self.save_choice {
            self.save_choice = save;
            self.core.send(Command::RememberSaveChoice { save });
        }
        let password = value("password").unwrap_or_default();
        self.core.send(Command::Answer { request_id, answer: Some(Answer::Password(password)), save });
        self.dialog = None;
        self.pending_action = None;
    }

    pub(super) fn set_form_error(&mut self, message: String) {
        if let Some(Dialog::Form(form)) = self.dialog.as_mut() {
            form.error = Some(message);
        }
    }

    pub(super) fn connect_to_selected(&mut self) {
        let Some(name) = self.connections.selected_entry().map(|entry| entry.name.clone()) else {
            return;
        };
        self.connect_profile(&name);
    }

    pub(super) fn connect_profile(&mut self, name: &str) -> bool {
        let known =
            self.connections.entries().iter().any(|entry| entry.name == name && !entry.source.is_orphan_labels());
        if !known {
            self.notifications.push(Severity::Warning, "That connection no longer exists");
            return false;
        }
        if let Some(session) = self.sessions.by_name(name) {
            let (id, healthy) = (session.id, session.healthy);
            if healthy {
                self.sessions.activate(id);
                return true;
            }
            self.core.send(Command::Disconnect { session: id });
        }
        self.core.send(Command::Connect { profile: name.to_string() });
        true
    }

    pub(super) fn disconnect_selected(&mut self) {
        let Some(entry) = self.connections.selected_entry().filter(|entry| !entry.source.is_orphan_labels()) else {
            return;
        };
        let Some(session) = self.sessions.by_name(&entry.name).map(|session| session.id) else {
            return;
        };

        self.core.send(Command::Disconnect { session });
    }

    pub(super) fn apply_connected(&mut self, session: u64, name: String, shell_available: bool) {
        self.connection_status = ConnectionStatus::Disconnected;
        self.session_names.insert(session, name.clone());
        self.last_failure.remove(&name);
        if self.sessions.activate(session) {
            return;
        }
        let placeholder_panel = PanelView::from_listing(VfsPath::root(), Vec::new());
        self.sessions.insert(session, name, shell_available, placeholder_panel);
    }
}

#[cfg(test)]
mod tests;

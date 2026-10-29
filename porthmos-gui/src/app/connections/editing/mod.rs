use iced::Task;
use iced::widget::{Id, operation};
use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};
use porthmos_core::{Command, Severity};

use super::super::{App, Message};
use crate::connections::form::FormState;
use crate::connections::{LabelsState, MissingChoice, Screen, announcement};

const NO_PROTOCOLS: &str = "No protocols are available in this build";
const SSH_CONFIG_DELETE: &str = "This connection is defined in ~/.ssh/config and can't be deleted here";

impl App {
    pub fn screen(&self) -> Option<&Screen> {
        self.connections_screen.as_ref()
    }

    pub fn form_input_id(key: &str) -> Id {
        Id::from(format!("form-{key}"))
    }

    pub(in crate::app) fn profiles_arrived(&mut self, entries: Vec<ConnectionEntry>) {
        if let Some(text) = announcement(&entries, &mut self.announced_orphans) {
            self.push_notice(Severity::Warning, format!("{text} They are marked \u{26A0} in the connection list."));
        }
        self.manager.replace(entries);
        if let Some(name) = self.reveal.take() {
            self.manager.reveal(&name);
        }
    }

    pub(in crate::app) fn profile_saved(&mut self) {
        if matches!(self.connections_screen, Some(Screen::Form(_) | Screen::Labels(_))) {
            self.set_screen(Some(Screen::List));
        }
    }

    pub(in crate::app) fn profile_rejected(&mut self, message: String) {
        self.reveal = None;
        match self.connections_screen.as_mut() {
            Some(Screen::Form(state)) => state.error = Some(message),
            Some(Screen::Labels(labels)) => labels.error = Some(message),
            _ => {}
        }
    }

    pub(in crate::app::connections) fn confirm_delete(&mut self) {
        if let Some(Screen::ConfirmDelete { name }) = self.connections_screen.take() {
            self.core.send(Command::DeleteProfile { name });
        }
        self.set_screen(Some(Screen::List));
    }

    fn open_add_form(&mut self) -> Task<Message> {
        match FormState::new(self.core.protocols(), None) {
            Some(state) => {
                self.set_screen(Some(Screen::Form(Box::new(state))));
                operation::focus(App::form_input_id("name"))
            }
            None => {
                self.push_notice(Severity::Warning, NO_PROTOCOLS.to_string());
                Task::none()
            }
        }
    }

    fn open_edit(&mut self) -> Task<Message> {
        let Some(entry) = self.manager.selected_entry().cloned() else {
            return Task::none();
        };
        match entry.source {
            ConnectionSource::Profile => match FormState::new(self.core.protocols(), Some(&entry)) {
                Some(state) => {
                    self.set_screen(Some(Screen::Form(Box::new(state))));
                    return operation::focus(App::form_input_id("name"));
                }
                None => self.push_notice(Severity::Warning, NO_PROTOCOLS.to_string()),
            },
            ConnectionSource::SshConfig => {
                self.set_screen(Some(Screen::Labels(LabelsState::for_entry(&entry))));
                return operation::focus(App::form_input_id("group"));
            }
            ConnectionSource::MissingSshHost | ConnectionSource::ShadowedSshHost => {
                let shadowed = entry.source == ConnectionSource::ShadowedSshHost;
                self.set_screen(Some(Screen::MissingHost { name: entry.name, shadowed }));
            }
        }
        Task::none()
    }

    fn ask_delete(&mut self) {
        let Some(entry) = self.manager.selected_entry() else {
            return;
        };
        match entry.source {
            ConnectionSource::Profile => {
                let name = entry.name.clone();
                self.set_screen(Some(Screen::ConfirmDelete { name }));
            }
            ConnectionSource::SshConfig => self.push_notice(Severity::Info, SSH_CONFIG_DELETE.to_string()),
            ConnectionSource::MissingSshHost | ConnectionSource::ShadowedSshHost => {}
        }
    }

    fn save_form(&mut self) {
        let Some(Screen::Form(state)) = self.connections_screen.as_mut() else {
            return;
        };
        if let Err(message) = state.check() {
            state.error = Some(message);
            return;
        }
        state.test = None;
        state.prompt = None;
        let original = state.original_name().map(str::to_string);
        let draft = state.draft();
        self.cancel_running_test();
        self.reveal = Some(draft.name.trim().to_string());
        self.core.send(Command::SaveProfile { original, draft: Box::new(draft) });
    }

    fn save_labels(&mut self) {
        let Some(Screen::Labels(labels)) = self.connections_screen.as_ref() else {
            return;
        };
        let (name, group, tags, forget) =
            (labels.name.clone(), labels.group.clone(), labels.tags.clone(), labels.forget_password);
        self.reveal = Some(name.clone());
        self.core.send(Command::SaveSshLabels { name: name.clone(), group, tags });
        if forget {
            self.core.send(Command::ForgetSshPassword { alias: name });
        }
    }

    fn choose_missing(&mut self, choice: MissingChoice) {
        let Some(Screen::MissingHost { name, .. }) = self.connections_screen.take() else {
            return;
        };
        match choice {
            MissingChoice::Move(to) => self.core.send(Command::MoveSshLabels { from: name, to }),
            MissingChoice::Forget => self.core.send(Command::ForgetSshLabels { name }),
        }
        self.set_screen(Some(Screen::List));
    }

    pub(in crate::app::connections) fn form_mut(&mut self) -> Option<&mut FormState> {
        match self.connections_screen.as_mut() {
            Some(Screen::Form(state)) => Some(state),
            _ => None,
        }
    }

    fn labels_mut(&mut self) -> Option<&mut LabelsState> {
        match self.connections_screen.as_mut() {
            Some(Screen::Labels(labels)) => {
                labels.error = None;
                Some(labels)
            }
            _ => None,
        }
    }

    pub(in crate::app) fn editing_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ManagerAddPressed => return self.open_add_form(),
            Message::ManagerEditPressed => return self.open_edit(),
            Message::ManagerDeletePressed => self.ask_delete(),
            Message::DeleteConfirmed => self.confirm_delete(),
            Message::ScreenCancelled => self.set_screen(Some(Screen::List)),
            Message::FormTextTyped(key, value) => {
                if let Some(state) = self.form_mut() {
                    state.set_text(&key, value);
                }
            }
            Message::FormChoiceChanged(key, value) => {
                if let Some(state) = self.form_mut() {
                    state.set_choice(&key, &value);
                }
            }
            Message::FormToggled(key, on) => {
                if let Some(state) = self.form_mut() {
                    state.set_toggle(&key, on);
                }
            }
            Message::FormSecretTyped(key, value) => {
                if let Some(state) = self.form_mut() {
                    state.set_secret(&key, value.into_string());
                }
            }
            Message::FormSecretForgotten(key) => {
                if let Some(state) = self.form_mut() {
                    state.forget_secret(&key);
                }
            }
            Message::FormSecretRestored(key) => {
                if let Some(state) = self.form_mut() {
                    state.undo_forget_secret(&key);
                }
            }
            Message::FormSavePressed => self.save_form(),
            Message::LabelsGroupTyped(group) => {
                if let Some(labels) = self.labels_mut() {
                    labels.group = group;
                }
            }
            Message::LabelsTagsTyped(tags) => {
                if let Some(labels) = self.labels_mut() {
                    labels.tags = tags;
                }
            }
            Message::LabelsForgetToggled(forget) => {
                if let Some(labels) = self.labels_mut() {
                    labels.forget_password = forget;
                }
            }
            Message::LabelsSavePressed => self.save_labels(),
            Message::MissingHostChosen(choice) => self.choose_missing(choice),
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests;

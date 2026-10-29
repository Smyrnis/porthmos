use iced::Task;
use iced::keyboard;
use iced::widget::{Id, operation};
use porthmos_core::profiles::ConnectionSource;
use porthmos_core::{Command, Severity};

use super::{App, Message};
use crate::connections::{Manager, ManagerButtons, Screen};
use crate::shortcuts::KeyPress;

impl App {
    pub fn manager_open(&self) -> bool {
        self.connections_screen.is_some()
    }

    pub fn manager(&self) -> &Manager {
        &self.manager
    }

    pub fn filter_input_id() -> Id {
        Id::new("manager-filter")
    }

    pub fn manager_buttons(&self) -> ManagerButtons {
        self.manager.buttons()
    }

    pub fn is_connected(&self, name: &str) -> bool {
        self.sessions.by_name(name).is_some()
    }

    pub fn protocol_name(&self, id: &str) -> String {
        self.core
            .protocols()
            .iter()
            .find(|info| info.id == id)
            .map_or_else(|| id.to_string(), |info| info.display_name.to_string())
    }

    pub(super) fn connect_profile(&mut self, name: &str) -> bool {
        let known = self.manager.entries().iter().any(|entry| {
            entry.name == name && matches!(entry.source, ConnectionSource::Profile | ConnectionSource::SshConfig)
        });
        if !known {
            self.push_notice(Severity::Warning, "That connection no longer exists".to_string());
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

    pub(in crate::app) fn set_screen(&mut self, screen: Option<Screen>) {
        if !matches!(screen, Some(Screen::Form(_))) {
            self.cancel_running_test();
        }
        self.connections_screen = screen;
    }

    fn connect_selected(&mut self) {
        if !self.manager.buttons().connect {
            return;
        }
        let Some(name) = self.manager.selected_entry().map(|entry| entry.name.clone()) else {
            return;
        };
        if self.connect_profile(&name) {
            self.set_screen(None);
        }
    }

    pub(in crate::app) fn manager_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ManagerToggled => {
                if self.connections_screen.is_some() {
                    self.set_screen(None);
                } else {
                    self.set_screen(Some(Screen::List));
                    self.core.send(Command::ListProfiles);
                    return operation::focus(App::filter_input_id());
                }
            }
            Message::ManagerClosed => self.set_screen(None),
            Message::ManagerFilterTyped(text) => self.manager.set_filter(text),
            Message::ManagerRowPressed(row) => self.manager.select_row(row),
            Message::ManagerRowActivated(row) => {
                self.manager.select_row(row);
                self.connect_selected();
            }
            Message::ManagerGroupToggled(path) => self.manager.toggle_group(&path),
            Message::ManagerConnect => self.connect_selected(),
            _ => {}
        }
        Task::none()
    }

    pub(in crate::app) fn manager_key(&mut self, press: &KeyPress) {
        use keyboard::Key;
        use keyboard::key::Named;
        if self.form_prompt_open() {
            if press.key == Key::Named(Named::Escape) {
                self.cancel_prompt();
            }
            return;
        }
        let on_list = matches!(self.connections_screen, Some(Screen::List));
        match &press.key {
            Key::Named(Named::Escape) if on_list => self.set_screen(None),
            Key::Named(Named::Escape) => self.set_screen(Some(Screen::List)),
            Key::Named(Named::Enter) => match self.connections_screen {
                Some(Screen::List) => self.connect_selected(),
                Some(Screen::ConfirmDelete { .. }) => self.confirm_delete(),
                _ => {}
            },
            Key::Named(Named::ArrowDown) if on_list => self.manager.move_selection(true),
            Key::Named(Named::ArrowUp) if on_list => self.manager.move_selection(false),
            _ => {}
        }
    }
}

mod editing;
mod prompts;
mod trial;

#[cfg(test)]
mod tests;

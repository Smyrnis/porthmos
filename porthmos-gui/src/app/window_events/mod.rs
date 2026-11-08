use std::path::PathBuf;

use iced::{Element, Size, Task, window};

use super::{App, Message};
use crate::shortcuts::KeyPress;
use crate::window_memory::WindowMemory;
use crate::windows::{WindowKind, Windows};

impl App {
    pub fn with_main_window(mut self, id: window::Id) -> App {
        let memory = self.windows.memory().clone();
        self.windows = Windows::new(id, self.window_size);
        self.windows.set_memory(memory);
        self
    }

    pub fn set_window_memory(&mut self, memory: WindowMemory, file: PathBuf) {
        self.windows.set_memory(memory);
        self.window_memory_file = Some(file);
    }

    pub(in crate::app) fn save_window_memory(&self) {
        let Some(file) = &self.window_memory_file else {
            return;
        };
        if let Err(error) = self.windows.memory().save(file) {
            tracing::warn!("the window sizes and positions could not be saved: {error:#}");
        }
    }

    #[cfg(test)]
    pub fn main_window(&self) -> window::Id {
        self.windows.main()
    }

    pub fn is_main(&self, id: window::Id) -> bool {
        id == self.windows.main()
    }

    pub fn view_of(&self, id: window::Id) -> Element<'_, Message> {
        match self.windows.kind_of(id) {
            Some(WindowKind::Settings) => {
                crate::view::settings::window(self).unwrap_or_else(|| crate::view::blank(self))
            }
            Some(WindowKind::Search) => crate::view::search::window(self).unwrap_or_else(|| crate::view::blank(self)),
            Some(WindowKind::Connections) => {
                crate::view::connections::window(self).unwrap_or_else(|| crate::view::blank(self))
            }
            Some(WindowKind::Main) => crate::view::root(self),
            None => crate::view::blank(self),
        }
    }

    pub fn title_of(&self, id: window::Id) -> String {
        self.windows.kind_of(id).unwrap_or(WindowKind::Main).title().to_string()
    }

    pub(in crate::app) fn window_close_requested(&mut self, id: window::Id) -> Task<Message> {
        match self.windows.kind_of(id) {
            Some(WindowKind::Main) => self.request_close(),
            Some(WindowKind::Settings) => self.close_settings(),
            Some(WindowKind::Search) => self.close_search(),
            Some(WindowKind::Connections) => {
                self.set_screen(None);
                Task::none()
            }
            None => window::close(id),
        }
    }

    pub(in crate::app) fn window_key(&mut self, id: window::Id, press: KeyPress) -> Task<Message> {
        match self.windows.kind_of(id) {
            Some(WindowKind::Main) => self.handle(Message::KeyPressed(press)),
            Some(WindowKind::Settings) => self.settings_window_key(&press),
            Some(WindowKind::Connections) => self.connections_window_key(&press),
            Some(WindowKind::Search) => self.search_window_key(&press),
            None => Task::none(),
        }
    }

    #[cfg(test)]
    pub fn window_id_of(&self, kind: WindowKind) -> Option<window::Id> {
        self.windows.id_of(kind)
    }

    fn connections_window_key(&mut self, press: &KeyPress) -> Task<Message> {
        use iced::keyboard::{Key, key::Named};
        use iced::widget::operation;
        if press.key == Key::Named(Named::Tab) {
            return if press.modifiers.shift() { operation::focus_previous() } else { operation::focus_next() };
        }
        self.manager_key(press);
        Task::none()
    }

    fn settings_window_key(&mut self, press: &KeyPress) -> Task<Message> {
        use iced::keyboard::{Key, key::Named};
        use iced::widget::operation;
        if press.key == Key::Named(Named::Tab) {
            return if press.modifiers.shift() { operation::focus_previous() } else { operation::focus_next() };
        }
        self.settings_key(press)
    }

    pub(in crate::app) fn window_sized(&mut self, id: window::Id, size: Size) {
        if self.windows.kind_of(id).is_none() {
            return;
        }
        self.windows.set_size(id, size);
        if self.is_main(id) {
            self.window_size = size;
        }
    }
}

#[cfg(test)]
mod tests;

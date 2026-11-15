use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::{Id, operation};
use iced::{Size, Task, window};
use porthmos_core::delete_prompt::THIS_COMPUTER;
use porthmos_core::{Command, Entry, Location};

use super::{App, Message};
use crate::geometry::{ROW_HEIGHT, scroll_to_show};
use crate::search::{PreviewSync, SearchState, result_parent};
use crate::shortcuts::KeyPress;
use crate::tools::Tool;
use crate::view::search::search_list_height;
use crate::windows::{SEARCH_WINDOW_SIZE, WindowKind};

impl App {
    pub fn search_state(&self) -> Option<&SearchState> {
        self.search.as_ref()
    }

    #[cfg(test)]
    pub fn search_scroll(&self) -> f32 {
        self.search_scroll
    }

    pub fn search_preview_scroll(&self) -> f32 {
        self.search_preview_scroll
    }

    pub fn search_preview_id() -> Id {
        Id::new("search-preview")
    }

    pub fn search_input_id() -> Id {
        Id::new("search-pattern")
    }

    pub fn search_list_id() -> Id {
        Id::new("search-list")
    }

    pub fn search_window_size(&self) -> Size {
        self.windows.id_of(WindowKind::Search).and_then(|id| self.windows.size_of(id)).unwrap_or(SEARCH_WINDOW_SIZE)
    }

    pub fn search_place(&self) -> String {
        match self.search.as_ref().map(SearchState::location) {
            Some(Location::Session(id)) => {
                self.sessions.by_id(id).map_or_else(|| id.to_string(), |session| session.name.clone())
            }
            _ => THIS_COMPUTER.to_string(),
        }
    }

    fn new_search(&self) -> Option<SearchState> {
        let side = self.active;
        let location = self.location_of(side)?;
        let root = self.panel_of(side).map(|panel| panel.path().clone())?;
        Some(SearchState::new(location, root).with_preview(self.gui_settings.search_preview))
    }

    pub(in crate::app) fn open_search(&mut self) -> Task<Message> {
        if let Some(id) = self.windows.id_of(WindowKind::Search) {
            return Task::batch([window::gain_focus(id), operation::focus(App::search_input_id())]);
        }
        let Some(state) = self.new_search() else {
            return Task::none();
        };
        let mut opening = Task::none();
        self.windows.open(WindowKind::Search, |settings| {
            let (id, task) = window::open(settings);
            opening = task.map(Message::WindowOpened);
            id
        });
        self.search = Some(state);
        self.search_scroll = 0.0;
        self.search_preview_scroll = 0.0;
        opening
    }

    pub(in crate::app) fn cancel_search_work(&mut self) {
        let Some(state) = &self.search else {
            return;
        };
        let had_preview = state.preview().is_some();
        self.core.send(Command::CancelSearch);
        if had_preview {
            self.cancel_search_preview();
        }
    }

    fn preview_tool_open(&self) -> bool {
        matches!(self.tool, Some(Tool::Preview(_)))
    }

    fn cancel_search_preview(&mut self) {
        if !self.preview_tool_open() {
            self.core.send(Command::CancelPreview);
        }
    }

    pub(in crate::app) fn search_preview_interrupted(&mut self) {
        if let Some(state) = self.search.as_mut() {
            state.reset_preview();
        }
    }

    pub(in crate::app) fn search_preview_resumed(&mut self) {
        self.search_preview_interrupted();
        self.sync_search_preview();
    }

    pub(in crate::app) fn apply_search_preview_setting(&mut self, enabled: bool) {
        let Some(state) = self.search.as_mut() else {
            return;
        };
        let had_preview = state.preview().is_some();
        state.set_preview_enabled(enabled);
        if had_preview {
            self.cancel_search_preview();
        }
        self.sync_search_preview();
    }

    pub(in crate::app) fn close_search(&mut self) -> Task<Message> {
        self.cancel_search_work();
        self.search = None;
        self.search_scroll = 0.0;
        self.search_preview_scroll = 0.0;
        match self.windows.id_of(WindowKind::Search) {
            Some(id) => {
                self.windows.close(id);
                window::close(id)
            }
            None => Task::none(),
        }
    }

    pub(in crate::app) fn close_search_on(&mut self, location: Location) -> Task<Message> {
        if self.search.as_ref().is_some_and(|state| state.location() == location) {
            return self.close_search();
        }
        Task::none()
    }

    pub(in crate::app) fn search_found(&mut self, entry: Entry) {
        if let Some(state) = self.search.as_mut() {
            state.found(entry);
        }
        self.sync_search_preview();
    }

    pub(in crate::app) fn search_done(&mut self, truncated: bool, incomplete: bool) {
        if let Some(state) = self.search.as_mut() {
            state.finished(truncated, incomplete);
        }
    }

    pub(in crate::app) fn search_failed(&mut self, message: String) {
        if let Some(state) = self.search.as_mut() {
            state.failed(message);
        }
    }

    fn sync_search_preview(&mut self) {
        if self.preview_tool_open() {
            return;
        }
        let request_id = self.next_preview_id;
        let Some(state) = self.search.as_mut() else {
            return;
        };
        let location = state.location();
        match state.sync_preview(request_id) {
            PreviewSync::Keep => {}
            PreviewSync::Cancel => {
                self.search_preview_scroll = 0.0;
                self.cancel_search_preview();
            }
            PreviewSync::Request { path } => {
                self.search_preview_scroll = 0.0;
                self.next_preview_id += 1;
                self.core.send(Command::PreviewHint { request_id, location, path });
            }
        }
    }

    fn open_search_result(&mut self) -> Task<Message> {
        let Some(state) = &self.search else {
            return Task::none();
        };
        let Some(entry) = state.selected().cloned() else {
            return Task::none();
        };
        let location = state.location();
        let parent = result_parent(&entry);
        if !self.show_location(location) {
            return Task::none();
        }
        self.reveal(location, parent, entry.path);
        window::gain_focus(self.windows.main())
    }

    fn move_search_selection(&mut self, down: bool) {
        if let Some(state) = self.search.as_mut() {
            state.move_selection(down);
        }
        self.sync_search_preview();
    }

    fn reveal_search_selection(&mut self) -> Task<Message> {
        let Some(state) = &self.search else {
            return Task::none();
        };
        let Some(row) = state.selected_row() else {
            return Task::none();
        };
        let viewport = search_list_height(self.search_window_size(), state.preview_enabled());
        let target = scroll_to_show(row, self.search_scroll, viewport, ROW_HEIGHT);
        if target == self.search_scroll {
            return Task::none();
        }
        self.search_scroll = target;
        operation::scroll_to(App::search_list_id(), operation::AbsoluteOffset { x: None, y: Some(target) })
    }

    pub(in crate::app) fn search_window_key(&mut self, press: &KeyPress) -> Task<Message> {
        match &press.key {
            Key::Named(Named::Escape) => return self.close_search(),
            Key::Named(Named::Enter) => return self.open_search_result(),
            Key::Named(direction @ (Named::ArrowDown | Named::ArrowUp)) => {
                self.move_search_selection(*direction == Named::ArrowDown);
                return self.reveal_search_selection();
            }
            Key::Character(_) | Key::Named(Named::Space) => return self.refocused_typing(press),
            _ => {}
        }
        Task::none()
    }

    fn refocused_typing(&mut self, press: &KeyPress) -> Task<Message> {
        let Some(text) = press.text.as_deref().filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
        else {
            return Task::none();
        };
        if press.modifiers.control() || press.modifiers.alt() || press.modifiers.logo() {
            return Task::none();
        }
        let Some(state) = &self.search else {
            return Task::none();
        };
        let typed = format!("{}{}", state.pattern(), text);
        Task::batch([self.search_message(Message::SearchTyped(typed)), operation::focus(App::search_input_id())])
    }

    pub(in crate::app) fn search_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SearchClosed => return self.close_search(),
            Message::SearchTyped(text) => {
                let Some(state) = self.search.as_mut() else {
                    return Task::none();
                };
                let had_preview = state.preview().is_some();
                let command = match state.set_pattern(text) {
                    Some(request) => {
                        Command::Search { location: request.location, root: request.root, pattern: request.pattern }
                    }
                    None => Command::CancelSearch,
                };
                self.core.send(command);
                if had_preview {
                    self.cancel_search_preview();
                }
                self.search_preview_scroll = 0.0;
                self.search_scroll = 0.0;
                return operation::scroll_to(
                    App::search_list_id(),
                    operation::AbsoluteOffset { x: None, y: Some(0.0) },
                );
            }
            Message::SearchRowPressed(row) => self.select_search_row(row),
            Message::SearchRowActivated(row) => {
                self.select_search_row(row);
                return self.open_search_result();
            }
            Message::SearchOpenPressed => return self.open_search_result(),
            Message::SearchPreviewScrolled(offset) => self.search_preview_scroll = offset,
            _ => {}
        }
        Task::none()
    }

    fn select_search_row(&mut self, row: usize) {
        if let Some(state) = self.search.as_mut() {
            state.select(row);
        }
        self.sync_search_preview();
    }
}

#[cfg(test)]
mod tests;

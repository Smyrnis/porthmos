use iced::Task;
use iced::keyboard;
use iced::widget::{Id, operation};
use porthmos_core::config::bookmarks::Bookmark;
use porthmos_core::history::HistoryEntry;
use porthmos_core::{Command, Entry, Location, Severity};

use super::{App, Message, PanelSide};
use crate::bookmarks::{BookmarksState, Goto, default_label, goto};
use crate::geometry::{ROW_HEIGHT, scroll_to_show};
use crate::history_view::{HistoryState, details};
use crate::modal::Modal;
use crate::search::{SearchState, result_parent};
use crate::shortcuts::KeyPress;
use crate::tools::Tool;
use crate::view::tools::list_height;

impl App {
    pub fn tool(&self) -> Option<&Tool> {
        self.tool.as_ref()
    }

    pub fn tool_scroll(&self) -> f32 {
        self.tool_scroll
    }

    #[cfg(test)]
    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    pub fn search_input_id() -> Id {
        Id::new("search-pattern")
    }

    pub fn history_filter_id() -> Id {
        Id::new("history-filter")
    }

    pub fn tool_list_id() -> Id {
        Id::new("tool-list")
    }

    pub(in crate::app) fn tool_input_id(&self) -> Option<Id> {
        self.tool.as_ref().and_then(|tool| match tool {
            Tool::Search(_) => Some(App::search_input_id()),
            Tool::Bookmarks(_) => None,
            Tool::History(_) => Some(App::history_filter_id()),
            Tool::Preview(_) => None,
            Tool::Palette(_) => Some(App::palette_input_id()),
            Tool::Settings(_) => None,
            Tool::Sync => None,
        })
    }

    pub(in crate::app) fn open_tool(&mut self, tool: Tool) {
        self.close_tool();
        self.tool = Some(tool);
        self.tool_scroll = 0.0;
        self.tool_version += 1;
    }

    pub(in crate::app) fn open_search(&mut self) {
        let side = self.active;
        let Some(location) = self.location_of(side) else {
            return;
        };
        let Some(root) = self.panel_of(side).map(|panel| panel.path().clone()) else {
            return;
        };
        self.open_tool(Tool::Search(SearchState::new(location, root)));
    }

    pub(in crate::app) fn open_bookmarks(&mut self) {
        let items = self.bookmarks.clone();
        self.open_tool(Tool::Bookmarks(BookmarksState::new(items)));
    }

    pub(in crate::app) fn open_history(&mut self) {
        self.open_tool(Tool::History(HistoryState::new(Vec::new())));
        self.core.send(Command::ListHistory);
    }

    pub(in crate::app) fn history_arrived(&mut self, entries: Vec<HistoryEntry>) {
        if let Some(Tool::History(state)) = self.tool.as_mut() {
            state.replace(entries);
        }
    }

    pub(in crate::app) fn open_bookmark_dialog(&mut self) {
        let side = self.active;
        let Some(location) = self.location_of(side) else {
            return;
        };
        let Some(path) = self.panel_of(side).map(|panel| panel.path().clone()) else {
            return;
        };
        let text = default_label(&path);
        self.modals.push(Modal::BookmarkName { location, path, text, error: None });
    }

    pub(in crate::app) fn close_tool(&mut self) {
        match self.tool.take() {
            Some(Tool::Search(_)) => self.core.send(Command::CancelSearch),
            Some(Tool::Preview(state)) if state.is_loading() => self.core.send(Command::CancelPreview),
            Some(Tool::Sync) => self.cancel_sync_review(),
            _ => {}
        }
    }

    pub(in crate::app) fn close_search_on(&mut self, location: Location) {
        if matches!(&self.tool, Some(Tool::Search(state)) if state.location() == location) {
            self.close_tool();
        }
    }

    pub(in crate::app) fn bookmarks_arrived(&mut self, items: Vec<Bookmark>) {
        if let Some(Tool::Bookmarks(state)) = self.tool.as_mut() {
            state.replace(items.clone());
        }
        self.bookmarks = items;
    }

    fn search_mut(&mut self) -> Option<&mut SearchState> {
        match self.tool.as_mut() {
            Some(Tool::Search(state)) => Some(state),
            _ => None,
        }
    }

    pub(in crate::app) fn search_found(&mut self, entry: Entry) {
        if let Some(state) = self.search_mut() {
            state.found(entry);
        }
    }

    pub(in crate::app) fn search_done(&mut self, truncated: bool) {
        if let Some(state) = self.search_mut() {
            state.finished(truncated);
        }
    }

    pub(in crate::app) fn search_failed(&mut self, message: String) {
        if let Some(state) = self.search_mut() {
            state.failed(message);
        }
    }

    pub(in crate::app) fn show_location(&mut self, location: Location) -> bool {
        match location {
            Location::Local => self.active = PanelSide::Local,
            Location::Session(id) => {
                if !self.sessions.activate(id) {
                    return false;
                }
                self.active = PanelSide::Remote;
            }
        }
        true
    }

    fn open_search_result(&mut self) {
        let Some(Tool::Search(state)) = &self.tool else {
            return;
        };
        let Some(entry) = state.selected().cloned() else {
            return;
        };
        let location = state.location();
        let parent = result_parent(&entry);
        self.close_tool();
        if self.show_location(location) {
            self.list(location, Some(parent));
        }
    }

    fn go_to_bookmark(&mut self) {
        let Some(Tool::Bookmarks(state)) = &self.tool else {
            return;
        };
        let Some((_, bookmark)) = state.selected() else {
            return;
        };
        match goto(bookmark, &self.sessions) {
            Goto::Local(path) => {
                self.close_tool();
                self.show_location(Location::Local);
                self.list(Location::Local, Some(path));
            }
            Goto::Session { id, path } => {
                self.close_tool();
                if self.show_location(Location::Session(id)) {
                    self.list(Location::Session(id), Some(path));
                }
            }
            Goto::NotConnected(host) => self.push_notice(Severity::Warning, format!("Connect to {host} first")),
        }
    }

    fn remove_bookmark(&mut self) {
        let Some(Tool::Bookmarks(state)) = &self.tool else {
            return;
        };
        if let Some((index, _)) = state.selected() {
            self.core.send(Command::RemoveBookmark { index });
        }
    }

    fn show_history_details(&mut self) {
        let Some(Tool::History(state)) = &self.tool else {
            return;
        };
        let Some(entry) = state.selected() else {
            return;
        };
        let (title, body) = details(entry, &chrono::Local);
        self.modals.push(Modal::Info { title, body });
    }

    fn ask_clear_history(&mut self) {
        if matches!(&self.tool, Some(Tool::History(state)) if state.total() > 0) {
            self.modals.push(Modal::ConfirmClearHistory);
        }
    }

    fn selected_row(&self) -> Option<usize> {
        match self.tool.as_ref()? {
            Tool::Search(state) => state.selected_row(),
            Tool::Bookmarks(state) => state.selected().map(|(row, _)| row),
            Tool::History(state) => state.selected_row(),
            Tool::Preview(_) => None,
            Tool::Palette(state) => (state.hit_count() > 0).then(|| state.selected()),
            Tool::Settings(_) => None,
            Tool::Sync => (self.sync_view.matched() > 0).then(|| self.sync_view.cursor()),
        }
    }

    fn move_tool_selection(&mut self, down: bool) {
        match self.tool.as_mut() {
            Some(Tool::Search(state)) => state.move_selection(down),
            Some(Tool::Bookmarks(state)) => state.move_selection(down),
            Some(Tool::History(state)) => state.move_selection(down),
            Some(Tool::Sync) => self.sync_view.move_cursor(if down { 1 } else { -1 }),
            Some(Tool::Preview(_) | Tool::Palette(_) | Tool::Settings(_)) | None => {}
        }
    }

    fn activate_tool_selection(&mut self) {
        match self.tool {
            Some(Tool::Search(_)) => self.open_search_result(),
            Some(Tool::Bookmarks(_)) => self.go_to_bookmark(),
            Some(Tool::History(_)) => self.show_history_details(),
            Some(Tool::Preview(_) | Tool::Palette(_) | Tool::Settings(_) | Tool::Sync) | None => {}
        }
    }

    fn select_tool_row(&mut self, row: usize) {
        match self.tool.as_mut() {
            Some(Tool::Search(state)) => state.select(row),
            Some(Tool::Bookmarks(state)) => state.select(row),
            Some(Tool::History(state)) => state.select(row),
            Some(Tool::Preview(_) | Tool::Palette(_) | Tool::Settings(_) | Tool::Sync) | None => {}
        }
    }

    pub(in crate::app) fn reveal_selection(&mut self) -> Task<Message> {
        let Some(tool) = self.tool.as_ref() else {
            return Task::none();
        };
        let Some(row) = self.selected_row() else {
            return Task::none();
        };
        let viewport = list_height(tool, self.window_size);
        let target = scroll_to_show(row, self.tool_scroll, viewport, ROW_HEIGHT);
        if target == self.tool_scroll {
            return Task::none();
        }
        self.tool_scroll = target;
        operation::scroll_to(App::tool_list_id(), operation::AbsoluteOffset { x: None, y: Some(target) })
    }

    pub(in crate::app) fn tool_key(&mut self, press: &KeyPress) -> Task<Message> {
        use keyboard::Key;
        use keyboard::key::Named;
        if matches!(self.tool, Some(Tool::Preview(_))) {
            return self.preview_key(press);
        }
        if matches!(self.tool, Some(Tool::Sync)) {
            return self.sync_key(press);
        }
        if matches!(self.tool, Some(Tool::Palette(_))) {
            return self.palette_key(press);
        }
        if matches!(self.tool, Some(Tool::Settings(_))) {
            return self.settings_key(press);
        }
        match &press.key {
            Key::Named(Named::Escape) => self.close_tool(),
            Key::Named(Named::Enter) => self.activate_tool_selection(),
            Key::Named(direction @ (Named::ArrowDown | Named::ArrowUp)) => {
                self.move_tool_selection(*direction == Named::ArrowDown);
                return self.reveal_selection();
            }
            _ => {}
        }
        Task::none()
    }

    pub(in crate::app) fn tool_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToolClosed => self.close_tool(),
            Message::SearchTyped(text) => {
                let Some(state) = self.search_mut() else {
                    return Task::none();
                };
                let command = match state.set_pattern(text) {
                    Some(request) => {
                        Command::Search { location: request.location, root: request.root, pattern: request.pattern }
                    }
                    None => Command::CancelSearch,
                };
                self.core.send(command);
                self.tool_scroll = 0.0;
                return operation::scroll_to(App::tool_list_id(), operation::AbsoluteOffset { x: None, y: Some(0.0) });
            }
            Message::HistoryFilterTyped(text) => {
                if let Some(Tool::History(state)) = self.tool.as_mut() {
                    state.set_filter(text);
                }
                self.tool_scroll = 0.0;
                return operation::scroll_to(App::tool_list_id(), operation::AbsoluteOffset { x: None, y: Some(0.0) });
            }
            Message::PreviewScrolled(offset) | Message::SyncScrolled(offset) => self.tool_scroll = offset,
            Message::HistoryDetailsPressed => self.show_history_details(),
            Message::HistoryClearPressed => self.ask_clear_history(),
            Message::SearchRowPressed(row) | Message::BookmarkRowPressed(row) | Message::HistoryRowPressed(row) => {
                self.select_tool_row(row)
            }
            Message::SearchRowActivated(row)
            | Message::BookmarkRowActivated(row)
            | Message::HistoryRowActivated(row) => {
                self.select_tool_row(row);
                self.activate_tool_selection();
            }
            Message::SearchOpenPressed | Message::BookmarkGoPressed => self.activate_tool_selection(),
            Message::BookmarkRemovePressed => self.remove_bookmark(),
            message => return self.sync_message(message),
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests;

use iced::Task;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::operation;
use porthmos_core::preview::Preview;
use porthmos_core::{Command, Entry};
use std::sync::Arc;

use super::{App, Message};
use crate::preview_view::PreviewState;
use crate::shortcuts::KeyPress;
use crate::tools::Tool;
use crate::view::preview::ROW_HEIGHT;
use crate::view::tools::list_height;

impl App {
    fn target_file(&self) -> Option<Entry> {
        let panel = self.panel_of(self.active)?;
        match panel.target_entries().as_slice() {
            [entry] if !entry.is_dir => Some(entry.clone()),
            _ => None,
        }
    }

    pub fn single_file(&self) -> bool {
        self.target_file().is_some()
    }

    pub(in crate::app) fn open_preview(&mut self) {
        let Some(entry) = self.target_file() else {
            return;
        };
        let Some(location) = self.location_of(self.active) else {
            return;
        };
        let request_id = self.next_preview_id;
        self.next_preview_id += 1;
        self.open_tool(Tool::Preview(PreviewState::loading(request_id, entry.name)));
        self.core.send(Command::Preview { request_id, location, path: entry.path });
    }

    pub(in crate::app) fn preview_arrived(&mut self, request_id: u64, result: Result<Arc<Preview>, String>) {
        if let Some(Tool::Preview(state)) = self.tool.as_mut() {
            state.finish(request_id, result);
        }
    }

    fn preview_limits(&self) -> Option<(f32, f32)> {
        let tool = self.tool.as_ref()?;
        let Tool::Preview(state) = tool else {
            return None;
        };
        let viewport = list_height(tool, self.window_size);
        let longest = (state.row_count() as f32 * ROW_HEIGHT - viewport).max(0.0);
        let page = ((viewport / ROW_HEIGHT).floor() * ROW_HEIGHT - ROW_HEIGHT).max(ROW_HEIGHT);
        Some((longest, page))
    }

    pub(in crate::app) fn preview_key(&mut self, press: &KeyPress) -> Task<Message> {
        let Some((longest, page)) = self.preview_limits() else {
            return Task::none();
        };
        let current = self.tool_scroll;
        let target = match &press.key {
            Key::Named(Named::Escape) => {
                self.close_tool();
                return Task::none();
            }
            Key::Named(Named::ArrowDown) => current + ROW_HEIGHT,
            Key::Named(Named::ArrowUp) => current - ROW_HEIGHT,
            Key::Named(Named::PageDown | Named::Space) => current + page,
            Key::Named(Named::PageUp) => current - page,
            Key::Named(Named::Home) => 0.0,
            Key::Named(Named::End) => longest,
            _ => return Task::none(),
        };
        let target = target.clamp(0.0, longest);
        self.tool_scroll = target;
        operation::scroll_to(App::tool_list_id(), operation::AbsoluteOffset { x: None, y: Some(target) })
    }
}

#[cfg(test)]
mod tests;

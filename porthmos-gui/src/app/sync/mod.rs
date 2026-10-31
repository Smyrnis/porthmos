use std::sync::Arc;

use iced::Task;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::widget::{Id, operation};
use porthmos_core::edit::printable;
use porthmos_core::sync::{SyncBy, SyncDirection, SyncOptions, SyncPlan};
use porthmos_core::{Command, Severity};

use super::{App, Message};
use crate::geometry::{ROW_HEIGHT, scroll_to_show};
use crate::modal::Modal;
use crate::shortcuts::KeyPress;
use crate::sync_view::SyncView;
use crate::tools::Tool;
use crate::view::text_fit::fit;
use crate::view::tools::sync_list_height;

const TITLE_CELLS: usize = 60;
const HELD_NOTICE: &str = "Sync preview ready; open it from More \u{2192} Synchronize\u{2026}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncChoice {
    Direction(SyncDirection),
    By(SyncBy),
    Subfolders(bool),
}

impl App {
    pub fn sync_filter_id() -> Id {
        Id::new("sync-filter")
    }

    pub fn sync_view(&self) -> &SyncView {
        &self.sync_view
    }

    pub fn sync_title(&self) -> String {
        let name =
            self.sync_view.plan().and_then(|plan| self.sessions.by_id(plan.session)).map(|session| &session.name);
        match name {
            Some(name) => format!("Synchronize with {}", fit(&printable(name), TITLE_CELLS)),
            None => "Synchronize".to_string(),
        }
    }

    pub fn sync_possible(&self) -> bool {
        self.sessions.active().is_some_and(|session| session.pane.is_listed())
    }

    pub(in crate::app) fn request_sync(&mut self) {
        if self.sync_view.has_plan() {
            if !matches!(self.tool, Some(Tool::Sync)) {
                self.open_tool(Tool::Sync);
            }
            return;
        }
        let Some(session) = self.sessions.active() else {
            return self.push_notice(Severity::Warning, "Connect to a remote server first".to_string());
        };
        if !session.pane.is_listed() {
            return self.push_notice(Severity::Warning, "Wait for the remote folder to finish loading".to_string());
        }
        self.modals.push(Modal::SyncOptions {
            session: session.id,
            name: session.name.clone(),
            offer_both: session.preserves_times,
            direction: SyncDirection::LocalToRemote,
            by: SyncBy::Time,
            subfolders: true,
            error: None,
        });
    }

    pub(in crate::app) fn sync_option_chosen(&mut self, choice: SyncChoice) {
        let Some(Modal::SyncOptions { offer_both, direction, by, subfolders, error, .. }) = self.modals.front_mut()
        else {
            return;
        };
        match choice {
            SyncChoice::Direction(SyncDirection::Both) if !*offer_both => return,
            SyncChoice::Direction(chosen) => *direction = chosen,
            SyncChoice::By(chosen) => *by = chosen,
            SyncChoice::Subfolders(chosen) => *subfolders = chosen,
        }
        *error = None;
    }

    pub(in crate::app) fn cycle_sync_direction(&mut self, forward: bool) {
        let Some(Modal::SyncOptions { offer_both, direction, error, .. }) = self.modals.front_mut() else {
            return;
        };
        let order: &[SyncDirection] = if *offer_both {
            &[SyncDirection::LocalToRemote, SyncDirection::RemoteToLocal, SyncDirection::Both]
        } else {
            &[SyncDirection::LocalToRemote, SyncDirection::RemoteToLocal]
        };
        let current = order.iter().position(|candidate| candidate == direction).unwrap_or(0);
        let next = if forward { (current + 1) % order.len() } else { (current + order.len() - 1) % order.len() };
        *direction = order[next];
        *error = None;
    }

    pub(in crate::app) fn sync_options_rejected(&mut self) -> bool {
        let Some(Modal::SyncOptions { direction, by, subfolders, error, .. }) = self.modals.front_mut() else {
            return false;
        };
        let options = SyncOptions { direction: *direction, by: *by, subfolders: *subfolders };
        match options.validate() {
            Ok(()) => false,
            Err(message) => {
                *error = Some(message.to_string());
                true
            }
        }
    }

    pub(in crate::app) fn start_sync(&mut self, session: porthmos_core::SessionId, options: SyncOptions) {
        let remote_dir = self.sessions.by_id(session).map(|live| live.pane.panel.path().clone());
        let Some(remote_dir) = remote_dir else {
            return self.push_notice(Severity::Warning, "That connection is no longer available".to_string());
        };
        let local_dir = self.local.panel.path().clone();
        self.core.send(Command::StartSync { session, local_dir, remote_dir, options });
    }
}

impl App {
    fn overlay_free(&self) -> bool {
        self.modals.is_empty()
            && self.connections_screen.is_none()
            && self.tool.is_none()
            && self.filtering.is_none()
            && !self.closing
    }

    fn sync_review_open(&self) -> bool {
        matches!(self.tool, Some(Tool::Sync))
    }

    pub(in crate::app) fn sync_plan_arrived(&mut self, plan: Arc<SyncPlan>) {
        if let Some(held) = self.sync_view.sync_id() {
            if held != plan.sync_id {
                self.core.send(Command::CancelSync { sync_id: plan.sync_id });
                self.push_notice(
                    Severity::Warning,
                    "A sync preview is already open; the new one was dropped".to_string(),
                );
            }
            return;
        }
        self.sync_view.load(plan);
        if self.overlay_free() {
            self.open_tool(Tool::Sync);
        } else {
            self.push_notice(Severity::Info, HELD_NOTICE.to_string());
        }
    }

    pub(in crate::app) fn sync_withdrawn(&mut self, sync_ids: &[u64]) {
        if !self.sync_view.sync_id().is_some_and(|id| sync_ids.contains(&id)) {
            return;
        }
        self.sync_view.clear();
        if self.sync_review_open() {
            self.tool = None;
        }
        self.push_notice(Severity::Info, "The sync plan was cancelled".to_string());
    }

    pub(in crate::app) fn cancel_sync_review(&mut self) {
        if let Some(sync_id) = self.sync_view.sync_id() {
            self.core.send(Command::CancelSync { sync_id });
        }
        self.sync_view.clear();
    }

    fn run_sync_review(&mut self) {
        let Some(sync_id) = self.sync_view.sync_id() else {
            return;
        };
        let choices = self.sync_view.visible_choices();
        if choices.is_empty() {
            let message = if self.sync_view.filter().is_some() {
                "Nothing is ticked in the rows shown"
            } else {
                "Nothing is ticked"
            };
            return self.push_notice(Severity::Warning, message.to_string());
        }
        self.core.send(Command::RunSync { sync_id, choices });
        self.sync_view.clear();
        self.tool = None;
    }

    fn sync_page(&self) -> isize {
        let rows = (sync_list_height(self.window_size) / ROW_HEIGHT).floor() as isize;
        (rows - 1).max(1)
    }

    fn sync_show_cursor_from_top(&mut self) -> Task<Message> {
        let target = if self.sync_view.matched() > 0 {
            scroll_to_show(self.sync_view.cursor(), 0.0, sync_list_height(self.window_size), ROW_HEIGHT)
        } else {
            0.0
        };
        self.tool_scroll = target;
        operation::scroll_to(App::tool_list_id(), operation::AbsoluteOffset { x: None, y: Some(target) })
    }

    pub(in crate::app) fn sync_key(&mut self, press: &KeyPress) -> Task<Message> {
        let everything = self.sync_view.matched() as isize;
        let delta = match &press.key {
            Key::Named(Named::Escape) => {
                self.close_tool();
                return Task::none();
            }
            Key::Named(Named::Space) => {
                self.sync_view.toggle();
                return Task::none();
            }
            Key::Named(Named::ArrowDown) => 1,
            Key::Named(Named::ArrowUp) => -1,
            Key::Named(Named::PageDown) => self.sync_page(),
            Key::Named(Named::PageUp) => -self.sync_page(),
            Key::Named(Named::Home) => -everything,
            Key::Named(Named::End) => everything,
            _ => return Task::none(),
        };
        self.sync_view.move_cursor(delta);
        self.reveal_selection()
    }

    pub(in crate::app) fn sync_message(&mut self, message: Message) -> Task<Message> {
        if !self.sync_review_open() {
            return Task::none();
        }
        match message {
            Message::SyncFilterTyped(text) => {
                self.sync_view.set_filter(text);
                return self.sync_show_cursor_from_top();
            }
            Message::SyncRowPressed(row) => {
                self.sync_view.select(row);
                self.sync_view.toggle();
            }
            Message::SyncTickAllPressed => self.sync_view.tick_all(),
            Message::SyncUntickAllPressed => self.sync_view.untick_all(),
            Message::SyncFlipPressed => self.sync_view.flip(),
            Message::SyncRunPressed => self.run_sync_review(),
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests;

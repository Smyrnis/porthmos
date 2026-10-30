use iced::Task;
use iced::keyboard::key::Named;
use iced::widget::{Id, operation};
use porthmos_core::edit::printable;
use porthmos_core::listing::Row;
use porthmos_core::profiles::ConnectionSource;
use porthmos_core::{Command, Location};

use super::{App, Message, PanelSide};
use crate::geometry::ROW_HEIGHT;
use crate::palette::{Entry, Group, PaletteState, Target};
use crate::shortcuts::{self, Action, KeyPress};
use crate::tools::Tool;
use crate::view::tools::list_height;

type Available = fn(&App) -> bool;

const COMMANDS: &[(Action, Available)] = &[
    (Action::Copy, |app| app.transfer_possible(app.active)),
    (Action::Move, |app| app.transfer_possible(app.active)),
    (Action::Preview, App::single_file),
    (Action::Edit, App::single_file),
    (Action::NewFolder, |app| app.panel_of(app.active).is_some()),
    (Action::Rename, |app| app.panel_of(app.active).is_some_and(|panel| panel.cursor_entry().is_some())),
    (Action::Delete, |app| app.panel_of(app.active).is_some_and(|panel| !panel.target_entries().is_empty())),
    (Action::Refresh, |app| app.location_of(app.active).is_some()),
    (Action::SelectAll, |_| true),
    (Action::ToggleHidden, |_| true),
    (Action::CycleSort, |_| true),
    (Action::StartFilter, |_| true),
    (Action::SwitchPanel, |_| true),
    (Action::NextTab, |_| true),
    (Action::PreviousTab, |_| true),
    (Action::ToggleQueue, |_| true),
    (Action::Search, |_| true),
    (Action::Bookmarks, |_| true),
    (Action::BookmarkHere, |_| true),
    (Action::History, |_| true),
    (Action::Sync, App::sync_possible),
    (Action::Terminal, App::terminal_possible),
    (Action::Settings, |_| true),
    (Action::CheckForUpdates, |_| true),
    (Action::Quit, |_| true),
];

const MANAGER_TITLE: &str = "Connection manager";

impl App {
    pub fn palette_input_id() -> Id {
        Id::new("palette-input")
    }

    pub(in crate::app) fn open_palette(&mut self) {
        if self.modals.front().is_some() {
            return;
        }
        let entries = self.palette_entries();
        self.open_tool(Tool::Palette(PaletteState::new(entries)));
        self.core.send(Command::ListProfiles);
    }

    pub(in crate::app) fn palette_profiles_changed(&mut self) {
        if !matches!(self.tool, Some(Tool::Palette(_))) {
            return;
        }
        let entries = self.palette_entries();
        if let Some(Tool::Palette(state)) = self.tool.as_mut() {
            state.replace_entries(entries);
        }
    }

    pub(in crate::app) fn palette_entries(&self) -> Vec<Entry> {
        let mut entries = Vec::new();
        for (action, available) in COMMANDS {
            if available(self) {
                entries.push(Entry {
                    group: Group::Commands,
                    label: action.title().to_string(),
                    hint: self.shortcuts.label_for(*action).map(|hint| printable(&hint)),
                    target: Target::Command(*action),
                });
            }
        }
        entries.push(Entry {
            group: Group::Commands,
            label: MANAGER_TITLE.to_string(),
            hint: None,
            target: Target::ConnectionManager,
        });
        for profile in self.manager.entries() {
            if matches!(profile.source, ConnectionSource::Profile | ConnectionSource::SshConfig) {
                entries.push(Entry {
                    group: Group::Hosts,
                    label: printable(&format!("Connect {}", profile.name)),
                    hint: Some(printable(&self.protocol_name(&profile.protocol))),
                    target: Target::Connect(profile.name.clone()),
                });
            }
        }
        for tab in self.sessions.names() {
            entries.push(Entry {
                group: Group::Hosts,
                label: printable(&format!("Disconnect {}", tab.name)),
                hint: Some("connected".to_string()),
                target: Target::Disconnect(tab.id),
            });
        }
        self.push_file_entries(&mut entries);
        entries
    }

    fn push_file_entries(&self, entries: &mut Vec<Entry>) {
        for side in [PanelSide::Local, PanelSide::Remote] {
            let (Some(location), Some(panel)) = (self.location_of(side), self.panel_of(side)) else {
                continue;
            };
            let hint = match side {
                PanelSide::Local => "local".to_string(),
                PanelSide::Remote => self.sessions.active().map(|session| printable(&session.name)).unwrap_or_default(),
            };
            for row in panel.rows() {
                let Row::Entry(entry) = row else {
                    continue;
                };
                let suffix = if entry.is_dir { "/" } else { "" };
                entries.push(Entry {
                    group: Group::Files,
                    label: printable(&format!("{}{suffix}", entry.name)),
                    hint: Some(hint.clone()),
                    target: Target::File { location, name: entry.name.clone() },
                });
            }
        }
    }

    pub(in crate::app) fn run_palette_target(&mut self, target: Target) -> Task<Message> {
        self.close_tool();
        match target {
            Target::Command(action) => return self.perform(action),
            Target::ConnectionManager => return self.manager_message(Message::ManagerToggled),
            Target::Connect(name) => {
                self.connect_profile(&name);
            }
            Target::Disconnect(session) => self.core.send(Command::Disconnect { session }),
            Target::File { location, name } => return self.show_file(location, &name),
        }
        Task::none()
    }

    fn show_file(&mut self, location: Location, name: &str) -> Task<Message> {
        if !self.show_location(location) {
            return Task::none();
        }
        let side = self.active;
        let Some(pane) = self.side_mut(side) else {
            return Task::none();
        };
        let row = pane.panel.rows().iter().position(|row| matches!(row, Row::Entry(entry) if entry.name == name));
        let Some(row) = row else {
            return Task::none();
        };
        pane.panel.set_cursor(row);
        self.reveal_cursor(side)
    }

    fn activate_palette(&mut self) -> Task<Message> {
        let Some(Tool::Palette(state)) = &self.tool else {
            return Task::none();
        };
        let Some(target) = state.selected_entry().map(|entry| entry.target.clone()) else {
            return Task::none();
        };
        self.run_palette_target(target)
    }

    fn palette_page(&self) -> isize {
        let Some(tool) = self.tool.as_ref() else {
            return 1;
        };
        ((list_height(tool, self.window_size) / ROW_HEIGHT) as isize - 1).max(1)
    }

    pub(in crate::app) fn palette_key(&mut self, press: &KeyPress) -> Task<Message> {
        let (key, modifiers) = shortcuts::effective(press);
        if self.shortcuts.action_for(&key, modifiers) == Some(Action::Palette) {
            self.close_tool();
            return Task::none();
        }
        let page = self.palette_page();
        let delta = match &press.key {
            iced::keyboard::Key::Named(Named::Escape) => {
                self.close_tool();
                return Task::none();
            }
            iced::keyboard::Key::Named(Named::Enter) => return self.activate_palette(),
            iced::keyboard::Key::Named(Named::ArrowDown) => 1,
            iced::keyboard::Key::Named(Named::ArrowUp) => -1,
            iced::keyboard::Key::Named(Named::PageDown) => page,
            iced::keyboard::Key::Named(Named::PageUp) => -page,
            _ => return Task::none(),
        };
        if let Some(Tool::Palette(state)) = self.tool.as_mut() {
            state.move_selection(delta);
        }
        self.reveal_selection()
    }

    pub(in crate::app) fn palette_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PaletteTyped(text) => {
                if let Some(Tool::Palette(state)) = self.tool.as_mut() {
                    state.set_query(text);
                }
                self.tool_scroll = 0.0;
                return operation::scroll_to(App::tool_list_id(), operation::AbsoluteOffset { x: None, y: Some(0.0) });
            }
            Message::PaletteRowPressed(row) => {
                if let Some(Tool::Palette(state)) = self.tool.as_mut() {
                    state.select(row);
                }
            }
            Message::PaletteRowActivated(row) => {
                if let Some(Tool::Palette(state)) = self.tool.as_mut() {
                    state.select(row);
                }
                return self.activate_palette();
            }
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests;

use crossterm::event::{KeyEvent, KeyModifiers};
use porthmos_core::Location;

use super::*;
use crate::{
    input::format_key_spec,
    palette::{Entry, Group, Target},
    widgets::{history_view::printable, panel_view::Row},
};

#[derive(Clone, Copy)]
enum Availability {
    Everywhere,
    On(Screen),
}

const ACTIONS: &[(Action, &str, Availability)] = &[
    (Action::Copy, "Copy to the other panel", Availability::On(Screen::Files)),
    (Action::Move, "Move to the other panel", Availability::On(Screen::Files)),
    (Action::Mkdir, "New folder", Availability::On(Screen::Files)),
    (Action::Delete, "Delete", Availability::On(Screen::Files)),
    (Action::Rename, "Rename", Availability::On(Screen::Files)),
    (Action::Edit, "Edit in $EDITOR", Availability::On(Screen::Files)),
    (Action::SwitchPanel, "Switch panel", Availability::On(Screen::Files)),
    (Action::ToggleHidden, "Show or hide hidden files", Availability::On(Screen::Files)),
    (Action::CycleSort, "Change the sort order", Availability::On(Screen::Files)),
    (Action::BookmarkHere, "Bookmark this folder", Availability::On(Screen::Files)),
    (Action::OpenBookmarks, "Bookmarks", Availability::On(Screen::Files)),
    (Action::OpenSearch, "Search files", Availability::On(Screen::Files)),
    (Action::OpenSync, "Synchronize folders", Availability::On(Screen::Files)),
    (Action::CycleSession, "Next session", Availability::On(Screen::Files)),
    (Action::OpenTerminal, "Open a shell on the server", Availability::On(Screen::Files)),
    (Action::Filter, "Filter files", Availability::On(Screen::Files)),
    (Action::Refresh, "Refresh", Availability::On(Screen::Files)),
    (Action::Open, "Connect", Availability::On(Screen::Connections)),
    (Action::Rename, "Edit connection", Availability::On(Screen::Connections)),
    (Action::DeleteConnection, "Delete connection", Availability::On(Screen::Connections)),
    (Action::Filter, "Filter connections", Availability::On(Screen::Connections)),
    (Action::Refresh, "Reload connections", Availability::On(Screen::Connections)),
    (Action::Pause, "Pause or resume the selected transfer", Availability::On(Screen::Transfers)),
    (Action::RowUp, "Move the selected transfer earlier", Availability::On(Screen::Transfers)),
    (Action::RowDown, "Move the selected transfer later", Availability::On(Screen::Transfers)),
    (Action::Open, "Retry the selected transfer", Availability::On(Screen::Transfers)),
    (Action::Delete, "Cancel the selected transfer", Availability::On(Screen::Transfers)),
    (Action::Refresh, "Clear finished transfers", Availability::On(Screen::Transfers)),
    (Action::Open, "Show the details", Availability::On(Screen::History)),
    (Action::Delete, "Clear the history", Availability::On(Screen::History)),
    (Action::Filter, "Filter history", Availability::On(Screen::History)),
    (Action::Filter, "Filter the preview", Availability::On(Screen::Sync)),
    (Action::Quit, "Quit", Availability::Everywhere),
    (Action::Help, "Help", Availability::Everywhere),
    (Action::OpenConnections, "Connections", Availability::Everywhere),
    (Action::AddConnection, "New connection", Availability::Everywhere),
    (Action::OpenTransfers, "Transfers", Availability::Everywhere),
    (Action::OpenHistory, "Transfer history", Availability::Everywhere),
    (Action::CancelTransfer, "Cancel all transfers", Availability::Everywhere),
    (Action::SpeedLimit, "Set the speed limit", Availability::Everywhere),
];

const PAGE: isize = 5;
const CONNECTED_HINT: &str = "connected";
const LOCAL_HINT: &str = "local";

impl App {
    pub(super) fn open_palette(&mut self) {
        if self.dialog.is_some() || self.help_visible || self.details_popup {
            return;
        }
        self.palette = Some(Palette::new(self.palette_entries()));
    }

    pub(super) fn opens_palette(&self, key: KeyEvent) -> bool {
        self.key_bindings.map_key(self.screen.scope(), key) == Action::OpenPalette
    }

    pub(super) fn apply_palette_key(&mut self, key: KeyEvent) {
        if self.opens_palette(key) {
            self.palette = None;
            return;
        }
        let Some(palette) = self.palette.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.palette = None,
            KeyCode::Enter => {
                let target = palette.selected_entry().map(|entry| entry.target.clone());
                if let Some(target) = target {
                    self.palette = None;
                    self.run_palette_target(target);
                }
            }
            KeyCode::Up => palette.move_selection(-1),
            KeyCode::Down => palette.move_selection(1),
            KeyCode::PageUp => palette.move_selection(-PAGE),
            KeyCode::PageDown => palette.move_selection(PAGE),
            KeyCode::Backspace => palette.backspace(),
            KeyCode::Delete => palette.delete(),
            KeyCode::Left => palette.move_caret(-1),
            KeyCode::Right => palette.move_caret(1),
            KeyCode::Home => palette.caret_home(),
            KeyCode::End => palette.caret_end(),
            KeyCode::Char(character) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                palette.type_char(character)
            }
            _ => {}
        }
    }

    fn run_palette_target(&mut self, target: Target) {
        match target {
            Target::Action(Action::AddConnection) if self.screen != Screen::Connections => {
                self.open_connections_screen();
                self.apply_action(Action::AddConnection);
            }
            Target::Action(action) => self.apply_action(action),
            Target::Connect(name) => {
                if self.connect_profile(&name) {
                    self.screen = Screen::Files;
                }
            }
            Target::Disconnect(name) => {
                if let Some(session) = self.sessions.by_name(&name).map(|session| session.id) {
                    self.core.send(Command::Disconnect { session });
                }
            }
            Target::File { location, name } => self.focus_file(location, &name),
        }
    }

    fn focus_file(&mut self, location: Location, name: &str) {
        let (focused, panel) = match location {
            Location::Local => (self.local.focus_name(name), ActivePanel::Local),
            Location::Session(id) => {
                let focused = self.sessions.by_id_mut(id).is_some_and(|session| session.panel.focus_name(name));
                if focused {
                    self.sessions.activate(id);
                }
                (focused, ActivePanel::Remote)
            }
        };
        if focused {
            self.screen = Screen::Files;
            self.active_panel = panel;
        }
    }

    pub(super) fn palette_entries(&self) -> Vec<Entry> {
        let mut entries = self.action_entries();
        entries.extend(self.host_entries());
        entries.extend(self.file_entries());
        entries
    }

    fn action_entries(&self) -> Vec<Entry> {
        let scope = self.screen.scope();
        let offered = |availability: &Availability| match availability {
            Availability::Everywhere => true,
            Availability::On(screen) => *screen == self.screen,
        };
        let on_screen = ACTIONS.iter().filter(|(_, _, availability)| matches!(availability, Availability::On(_)));
        let everywhere = ACTIONS.iter().filter(|(_, _, availability)| matches!(availability, Availability::Everywhere));
        on_screen
            .chain(everywhere)
            .filter(|(_, _, availability)| offered(availability))
            .map(|(action, label, _)| Entry {
                group: Group::Actions,
                label: (*label).to_string(),
                hint: self.key_bindings.key_for(scope, *action).map(format_key_spec),
                target: Target::Action(*action),
            })
            .collect()
    }

    fn host_entries(&self) -> Vec<Entry> {
        let protocols = self.core.protocols();
        let connect = self.connections.entries().iter().filter(|entry| !entry.source.is_orphan_labels()).map(|entry| {
            let protocol = protocols
                .iter()
                .find(|info| info.id == entry.protocol)
                .map_or(entry.protocol.as_str(), |info| info.display_name);
            Entry {
                group: Group::Hosts,
                label: format!("Connect {}", printable(&entry.name)),
                hint: Some(printable(protocol)),
                target: Target::Connect(entry.name.clone()),
            }
        });
        let disconnect = self.sessions.iter().map(|session| Entry {
            group: Group::Hosts,
            label: format!("Disconnect {}", printable(&session.name)),
            hint: Some(CONNECTED_HINT.to_string()),
            target: Target::Disconnect(session.name.clone()),
        });
        connect.chain(disconnect).collect()
    }

    fn file_entries(&self) -> Vec<Entry> {
        let mut entries = panel_file_entries(&self.local, Location::Local, LOCAL_HINT.to_string());
        if let Some(session) = self.sessions.active() {
            entries.extend(panel_file_entries(&session.panel, Location::Session(session.id), printable(&session.name)));
        }
        entries
    }
}

fn panel_file_entries(panel: &PanelView, location: Location, hint: String) -> Vec<Entry> {
    panel
        .rows()
        .iter()
        .filter_map(|row| match row {
            Row::Entry(entry) => Some(entry),
            Row::Parent => None,
        })
        .map(|entry| {
            let suffix = if entry.is_dir { "/" } else { "" };
            Entry {
                group: Group::Files,
                label: format!("{}{suffix}", printable(&entry.name)),
                hint: Some(hint.clone()),
                target: Target::File { location, name: entry.name.clone() },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;

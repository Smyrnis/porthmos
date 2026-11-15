use crossterm::event::{KeyEvent, KeyModifiers};
use porthmos_core::i18n;
use porthmos_core::{Location, offer_summary};

use super::*;
use crate::{
    input::format_key_spec,
    palette::{Entry, Group, Target, reopen_label},
    widgets::{history_view::printable, panel_view::Row},
};

#[derive(Clone, Copy)]
enum Availability {
    Everywhere,
    On(Screen),
}

const ACTIONS: &[(Action, &str, Availability)] = &[
    (Action::Copy, "palette.action.copy_to_the_other_panel", Availability::On(Screen::Files)),
    (Action::Move, "palette.action.move_to_the_other_panel", Availability::On(Screen::Files)),
    (Action::Preview, "palette.action.preview_the_file", Availability::On(Screen::Files)),
    (Action::Mkdir, "palette.action.new_folder", Availability::On(Screen::Files)),
    (Action::Delete, "palette.action.delete", Availability::On(Screen::Files)),
    (Action::Rename, "palette.action.rename", Availability::On(Screen::Files)),
    (Action::Permissions, "palette.action.change_permissions", Availability::On(Screen::Files)),
    (Action::Properties, "palette.action.size_and_sha_256_checksum", Availability::On(Screen::Files)),
    (Action::Edit, "palette.action.edit_in_editor", Availability::On(Screen::Files)),
    (Action::SwitchPanel, "palette.action.switch_panel", Availability::On(Screen::Files)),
    (Action::ToggleHidden, "palette.action.show_or_hide_hidden_files", Availability::On(Screen::Files)),
    (Action::CycleSort, "palette.action.change_the_sort_order", Availability::On(Screen::Files)),
    (Action::BookmarkHere, "palette.action.bookmark_this_folder", Availability::On(Screen::Files)),
    (Action::OpenBookmarks, "palette.action.bookmarks", Availability::On(Screen::Files)),
    (Action::OpenSearch, "palette.action.search_files", Availability::On(Screen::Files)),
    (Action::GoToPath, "palette.action.go_to_folder_path", Availability::On(Screen::Files)),
    (Action::OpenSync, "palette.action.synchronize_folders", Availability::On(Screen::Files)),
    (Action::CompareFolders, "palette.action.compare_folders", Availability::On(Screen::Files)),
    (Action::CycleSession, "palette.action.next_session", Availability::On(Screen::Files)),
    (Action::OpenTerminal, "palette.action.open_a_terminal_in_the_active_folder", Availability::On(Screen::Files)),
    (Action::Filter, "palette.action.filter_files", Availability::On(Screen::Files)),
    (Action::Refresh, "palette.action.refresh", Availability::On(Screen::Files)),
    (Action::Open, "palette.action.connect", Availability::On(Screen::Connections)),
    (Action::Rename, "palette.action.edit_connection", Availability::On(Screen::Connections)),
    (Action::DeleteConnection, "palette.action.delete_connection", Availability::On(Screen::Connections)),
    (Action::Filter, "palette.action.filter_connections", Availability::On(Screen::Connections)),
    (Action::Refresh, "palette.action.reload_connections", Availability::On(Screen::Connections)),
    (Action::Pause, "palette.action.pause_or_resume_the_selected_transfer", Availability::On(Screen::Transfers)),
    (Action::RowUp, "palette.action.move_the_selected_transfer_earlier", Availability::On(Screen::Transfers)),
    (Action::RowDown, "palette.action.move_the_selected_transfer_later", Availability::On(Screen::Transfers)),
    (Action::Open, "palette.action.retry_the_selected_transfer", Availability::On(Screen::Transfers)),
    (Action::Delete, "palette.action.cancel_the_selected_transfer", Availability::On(Screen::Transfers)),
    (Action::Refresh, "palette.action.clear_finished_transfers", Availability::On(Screen::Transfers)),
    (Action::Open, "palette.action.show_the_details", Availability::On(Screen::History)),
    (Action::Delete, "palette.action.clear_the_history", Availability::On(Screen::History)),
    (Action::Filter, "palette.action.filter_history", Availability::On(Screen::History)),
    (Action::Filter, "palette.action.filter_the_preview", Availability::On(Screen::Sync)),
    (Action::Quit, "palette.action.quit", Availability::Everywhere),
    (Action::Help, "palette.action.help", Availability::Everywhere),
    (Action::OpenConnections, "palette.action.connections", Availability::Everywhere),
    (Action::AddConnection, "palette.action.new_connection", Availability::Everywhere),
    (Action::OpenTransfers, "palette.action.transfers", Availability::Everywhere),
    (Action::OpenHistory, "palette.action.transfer_history", Availability::Everywhere),
    (Action::CancelTransfer, "palette.action.cancel_all_transfers", Availability::Everywhere),
    (Action::SpeedLimit, "palette.action.set_the_speed_limit", Availability::Everywhere),
    (Action::CheckForUpdates, "palette.action.check_for_updates", Availability::Everywhere),
    (
        Action::ImportConnections,
        "palette.action.import_connections_from_filezilla_winscp_or_putt",
        Availability::Everywhere,
    ),
];

const PAGE: isize = 5;

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
            Target::Reopen => {
                self.reopen_connections();
                self.screen = Screen::Files;
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
                label: i18n::text(label),
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
                label: i18n::text_with("palette.connect", &[("name", &printable(&entry.name))]),
                hint: Some(printable(protocol)),
                target: Target::Connect(entry.name.clone()),
            }
        });
        let disconnect = self.sessions.iter().map(|session| Entry {
            group: Group::Hosts,
            label: i18n::text_with("palette.disconnect", &[("name", &printable(&session.name))]),
            hint: Some(i18n::text("palette.hint.connected")),
            target: Target::Disconnect(session.name.clone()),
        });
        let reopen = (!self.reopen_offer.is_empty()).then(|| Entry {
            group: Group::Hosts,
            label: reopen_label(),
            hint: Some(printable(&offer_summary(&self.reopen_offer))),
            target: Target::Reopen,
        });
        reopen.into_iter().chain(connect).chain(disconnect).collect()
    }

    fn file_entries(&self) -> Vec<Entry> {
        let mut entries = panel_file_entries(&self.local, Location::Local, i18n::text("panel.local"));
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

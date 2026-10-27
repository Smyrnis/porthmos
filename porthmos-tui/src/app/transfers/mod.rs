use porthmos_core::Entry;

use super::*;

impl App {
    pub(super) fn start_copy(&mut self) {
        self.start_transfer(false);
    }

    pub(super) fn start_move(&mut self) {
        self.start_transfer(true);
    }

    fn start_transfer(&mut self, moving: bool) {
        if self.screen != Screen::Files {
            return;
        }

        match self.active_panel {
            ActivePanel::Local => self.send_to_remote(moving),
            ActivePanel::Remote => self.send_to_local(moving),
        }
    }

    fn transfer_command(moving: bool, from: Location, entries: Vec<Entry>, to: Location, dest_dir: VfsPath) -> Command {
        if moving {
            Command::Move { from, entries, to, dest_dir }
        } else {
            Command::Copy { from, entries, to, dest_dir }
        }
    }

    fn send_to_remote(&mut self, moving: bool) {
        let Some(session) = self.sessions.active() else {
            self.notifications.push(Severity::Warning, "Connect to a remote server first");
            return;
        };
        let entries = self.local.target_entries();
        if entries.is_empty() {
            return;
        }
        self.core.send(Self::transfer_command(
            moving,
            Location::Local,
            entries,
            Location::Session(session.id),
            session.panel.path().clone(),
        ));
    }

    fn send_to_local(&mut self, moving: bool) {
        let Some(session) = self.sessions.active() else {
            return;
        };
        let entries = session.panel.target_entries();
        if entries.is_empty() {
            return;
        }
        self.core.send(Self::transfer_command(
            moving,
            Location::Session(session.id),
            entries,
            Location::Local,
            self.local.path().clone(),
        ));
    }
}

#[cfg(test)]
mod tests;

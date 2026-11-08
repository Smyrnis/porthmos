use super::*;

impl App {
    pub(super) fn open_search_screen(&mut self) {
        if self.screen != Screen::Files {
            return;
        }

        let Some(location) = self.active_location() else {
            self.notifications.push(Severity::Warning, "Connect to a remote server first");
            return;
        };

        let place = match location {
            Location::Local => porthmos_core::delete_prompt::THIS_COMPUTER.to_string(),
            Location::Session(id) => self.sessions.by_id(id).map(|session| session.name.clone()).unwrap_or_default(),
        };
        let folder = self.panel(location).map(|panel| panel.path().to_string()).unwrap_or_default();
        let mut view = SearchView::new();
        view.scope = format!("{place} \u{b7} {folder}");
        self.search = Some(SearchSession { view, location });
        self.screen = Screen::Search;
    }

    pub(super) fn apply_search_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.search.is_some() {
                self.core.send(Command::CancelSearch);
            }
            return;
        }

        let Some(session) = self.search.as_mut() else {
            return;
        };

        match session.view.handle_key(key) {
            SearchOutcome::Cancel => {
                self.cancel_search_preview();
                self.search = None;
                self.core.send(Command::CancelSearch);
                self.screen = Screen::Files;
                self.open_next_conflict_prompt();
            }
            SearchOutcome::PatternChanged => self.restart_search(),
            SearchOutcome::Open => self.open_selected_search_result(),
            SearchOutcome::Pending => self.sync_search_pane(),
        }
    }

    fn cancel_search_preview(&mut self) {
        if self.search.as_ref().is_some_and(|session| session.view.pane.is_some()) {
            self.core.send(Command::CancelPreview);
        }
    }

    pub(super) fn sync_search_pane(&mut self) {
        let enabled = self.search_preview;
        let Some(session) = self.search.as_mut() else {
            return;
        };
        let location = session.location;
        match session.view.sync_pane(enabled) {
            PaneSync::Keep => {}
            PaneSync::Cancel => self.core.send(Command::CancelPreview),
            PaneSync::Request { path } => {
                self.next_preview_id += 1;
                self.core.send(Command::PreviewHint { request_id: self.next_preview_id, location, path });
            }
        }
    }

    fn restart_search(&mut self) {
        let Some(session) = self.search.as_mut() else {
            return;
        };
        let had_pane = session.view.pane.is_some();
        session.view.start();
        let pattern = session.view.pattern.clone();
        let location = session.location;
        if had_pane {
            self.core.send(Command::CancelPreview);
        }
        if pattern.is_empty() {
            self.core.send(Command::CancelSearch);
            return;
        }

        let root = self.panel(location).map(|panel| panel.path().clone()).unwrap_or_default();
        self.core.send(Command::Search { location, root, pattern: format!("*{pattern}*") });
    }

    fn open_selected_search_result(&mut self) {
        let Some(session) = self.search.as_ref() else {
            return;
        };
        let Some(entry) = session.view.selected_entry().cloned() else {
            return;
        };
        let location = session.location;

        self.cancel_search_preview();
        self.search = None;
        self.core.send(Command::CancelSearch);
        self.screen = Screen::Files;
        self.open_next_conflict_prompt();
        self.active_panel = match location {
            Location::Local => ActivePanel::Local,
            Location::Session(id) => {
                self.sessions.activate(id);
                ActivePanel::Remote
            }
        };
        self.core.send(Command::Reveal { location, path: entry.path });
    }
}

#[cfg(test)]
mod tests;

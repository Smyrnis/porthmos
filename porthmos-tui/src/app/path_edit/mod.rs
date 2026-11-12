use super::*;
use crate::widgets::path_field::{PathField, PathOutcome};
use porthmos_core::{Completion, CompletionStep};

pub(super) struct PathEdit {
    pub(super) location: Location,
    pub(super) field: PathField,
    pub(super) waiting: bool,
    pub(super) completing: bool,
    pub(super) listing: Option<Completion>,
}

impl App {
    pub(super) fn open_path_edit(&mut self) {
        if self.screen != Screen::Files {
            return;
        }
        let Some(location) = self.active_location() else {
            self.notifications.push(Severity::Warning, "Connect to a remote server first");
            return;
        };
        let Some(panel) = self.panel(location) else {
            return;
        };
        let text = match location {
            Location::Local => render::abbreviate_home(panel.path(), self.home.as_ref()),
            Location::Session(_) => panel.path().to_string(),
        };
        self.path_edit =
            Some(PathEdit { location, field: PathField::new(text), waiting: false, completing: false, listing: None });
    }

    pub(super) fn editing_field(&self, location: Location) -> Option<&PathField> {
        self.path_edit.as_ref().filter(|edit| edit.location == location).map(|edit| &edit.field)
    }

    pub(super) fn drop_stale_path_edit(&mut self) -> bool {
        let Some(edit) = self.path_edit.as_ref() else {
            return false;
        };
        let session_gone = match edit.location {
            Location::Local => false,
            Location::Session(id) => self.sessions.active_id() != Some(id),
        };
        let stale = self.screen != Screen::Files || session_gone;
        if stale {
            self.path_edit = None;
        }
        stale
    }

    pub(super) fn apply_path_key(&mut self, key: KeyEvent) {
        let Some(edit) = self.path_edit.as_mut() else {
            return;
        };
        match edit.field.handle_key(key) {
            PathOutcome::Pending => edit.listing = None,
            PathOutcome::Complete => self.complete_path_edit(),
            PathOutcome::Cancel if edit.listing.is_some() => edit.listing = None,
            PathOutcome::Cancel => self.path_edit = None,
            PathOutcome::Submit if edit.waiting => edit.listing = None,
            PathOutcome::Submit => {
                edit.listing = None;
                self.submit_path_edit();
            }
        }
    }

    fn complete_path_edit(&mut self) {
        let Some((location, text, busy)) = self
            .path_edit
            .as_ref()
            .map(|edit| (edit.location, edit.field.text.clone(), edit.completing || edit.waiting))
        else {
            return;
        };
        let Some(from) = self.panel(location).map(|panel| panel.path().clone()).filter(|_| !busy) else {
            return;
        };
        if let Some(edit) = self.path_edit.as_mut() {
            edit.completing = true;
            edit.listing = None;
        }
        self.core.send(Command::CompletePath { location, from, text });
    }

    pub(super) fn apply_path_completion(&mut self, location: Location, text: String, completion: Completion) {
        let Some(edit) = self.path_edit.as_mut().filter(|edit| edit.location == location) else {
            return;
        };
        edit.completing = false;
        if edit.field.text != text {
            return;
        }
        match completion.step(&text) {
            CompletionStep::Replace(completed) => {
                edit.field = PathField::new(completed);
                edit.listing = None;
            }
            CompletionStep::Show(_) => edit.listing = Some(completion),
            CompletionStep::Nothing => {}
        }
    }

    pub(super) fn path_listing(&self, location: Location) -> Option<&Completion> {
        self.path_edit.as_ref().filter(|edit| edit.location == location).and_then(|edit| edit.listing.as_ref())
    }

    fn submit_path_edit(&mut self) {
        let Some(edit) = self.path_edit.as_mut() else {
            return;
        };
        let text = edit.field.text.trim().to_string();
        let location = edit.location;
        if text.is_empty() {
            self.path_edit = None;
            return;
        }
        edit.waiting = true;
        let Some(from) = self.panel(location).map(|panel| panel.path().clone()) else {
            self.path_edit = None;
            return;
        };
        self.core.send(Command::GoToPath { location, from, text });
    }

    pub(super) fn finish_path_edit(&mut self, location: Location) {
        if self.path_edit.as_ref().is_some_and(|edit| edit.waiting && edit.location == location) {
            self.path_edit = None;
        }
    }

    pub(super) fn reject_path_edit(&mut self, location: Location) {
        if let Some(edit) = self.path_edit.as_mut().filter(|edit| edit.location == location) {
            edit.waiting = false;
        }
    }
}

#[cfg(test)]
mod tests;

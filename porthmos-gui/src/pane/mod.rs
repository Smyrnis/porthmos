use porthmos_core::listing::SortSpec;
use porthmos_core::{Entry, VfsPath};

use crate::navigation::History;
use crate::panel::Panel;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expected {
    Current,
    Anything,
    Folder(VfsPath),
}

#[derive(Debug, Clone)]
pub struct Pane {
    pub panel: Panel,
    pub scroll: f32,
    pub history: History,
    expected: Expected,
    recording: bool,
    listed: bool,
}

impl Pane {
    pub fn new(path: VfsPath) -> Pane {
        Pane {
            panel: Panel::new(path, SortSpec::default(), false),
            scroll: 0.0,
            history: History::new(),
            expected: Expected::Current,
            recording: true,
            listed: false,
        }
    }

    pub fn awaiting_first_listing() -> Pane {
        Pane {
            panel: Panel::new(VfsPath::root(), SortSpec::default(), false),
            scroll: 0.0,
            history: History::new(),
            expected: Expected::Anything,
            recording: true,
            listed: false,
        }
    }

    pub fn is_listed(&self) -> bool {
        self.listed
    }

    pub fn expect_unrecorded(&mut self, path: Option<VfsPath>) {
        self.expect(path);
        self.recording = false;
    }

    pub fn expect(&mut self, path: Option<VfsPath>) {
        self.recording = true;
        self.expected = match path {
            Some(path) => Expected::Folder(path),
            None => Expected::Anything,
        };
    }

    pub fn accept(&mut self, path: VfsPath, entries: Vec<Entry>) -> bool {
        let requested = match &self.expected {
            Expected::Anything => true,
            Expected::Folder(folder) => folder == &path,
            Expected::Current => false,
        };
        if !requested && &path != self.panel.path() {
            return false;
        }
        let navigating = matches!(self.expected, Expected::Folder(_));
        if requested {
            self.expected = Expected::Current;
        }
        if &path != self.panel.path() {
            self.scroll = 0.0;
            if requested && navigating && self.recording {
                self.history.visit(self.panel.path().clone());
            }
        }
        if requested {
            self.recording = true;
        }
        self.panel.replace_listing(path, entries);
        self.listed = true;
        true
    }
}

#[cfg(test)]
mod tests;

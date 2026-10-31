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
    pending_focus: Option<(VfsPath, String)>,
    reveal_cursor: bool,
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
            pending_focus: None,
            reveal_cursor: false,
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
            pending_focus: None,
            reveal_cursor: false,
        }
    }

    pub fn focus_when_listed(&mut self, folder: VfsPath, name: String) {
        self.pending_focus = Some((folder, name));
    }

    pub fn take_reveal_request(&mut self) -> bool {
        std::mem::take(&mut self.reveal_cursor)
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
            Some(path) => {
                self.focus_the_folder_being_left(&path);
                Expected::Folder(path)
            }
            None => Expected::Anything,
        };
    }

    fn focus_the_folder_being_left(&mut self, target: &VfsPath) {
        let current = self.panel.path();
        let left = current
            .ancestors()
            .find(|candidate| candidate.parent().as_ref() == Some(target))
            .and_then(|child| child.file_name().map(|name| name.to_string_lossy().to_string()));
        self.pending_focus = left.map(|name| (target.clone(), name));
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
        self.panel.replace_listing(path.clone(), entries);
        self.listed = true;
        if let Some((_, name)) = self.pending_focus.take_if(|(folder, _)| folder == &path) {
            self.reveal_cursor = self.panel.focus_name(&name);
        }
        true
    }
}

#[cfg(test)]
mod tests;

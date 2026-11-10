use std::collections::VecDeque;

use porthmos_core::delete_prompt::count::{Contents, contents_line};
use porthmos_core::edit::{EditQuestionKind, printable};
use porthmos_core::sync::{SyncBy, SyncDirection};
use porthmos_core::{Entry, Location, SessionId, VfsPath};

use crate::app::PanelSide;
use crate::conflicts::ConflictView;
use crate::dialogs::Dialog;
use crate::view::text_fit::fit;

const NAME_CELLS: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteCount {
    pub request_id: u64,
    pub contents: Contents,
}

#[derive(Debug, Clone)]
pub enum Modal {
    Question(Dialog),
    NewFolder {
        side: PanelSide,
        text: String,
        error: Option<String>,
    },
    Rename {
        side: PanelSide,
        original: Entry,
        text: String,
        error: Option<String>,
    },
    BookmarkName {
        location: Location,
        path: VfsPath,
        text: String,
        error: Option<String>,
    },
    Info {
        title: String,
        body: String,
    },
    ConfirmClearHistory,
    ConfirmDelete {
        side: PanelSide,
        place: String,
        entries: Vec<Entry>,
        count: Option<DeleteCount>,
    },
    Conflict {
        view: ConflictView,
        apply_to_rest: bool,
    },
    EditQuestion {
        edit_id: u64,
        name: String,
        kind: EditQuestionKind,
    },
    EditWaiting {
        edit_id: u64,
        name: String,
    },
    ConfirmQuit {
        transfers: bool,
        edits: bool,
    },
    SyncOptions {
        session: SessionId,
        name: String,
        offer_both: bool,
        direction: SyncDirection,
        by: SyncBy,
        subfolders: bool,
        error: Option<String>,
    },
}

impl Modal {
    pub fn is_risky_to_accept(&self) -> bool {
        matches!(
            self,
            Modal::Question(Dialog::Confirm { risky: true, .. })
                | Modal::ConfirmDelete { .. }
                | Modal::ConfirmClearHistory
                | Modal::ConfirmQuit { .. }
        )
    }
}

#[derive(Debug, Default)]
pub struct ModalQueue {
    items: VecDeque<Modal>,
    version: u64,
}

impl ModalQueue {
    pub fn push(&mut self, modal: Modal) {
        if self.items.is_empty() {
            self.version += 1;
        }
        self.items.push_back(modal);
    }

    pub fn front(&self) -> Option<&Modal> {
        self.items.front()
    }

    pub fn front_mut(&mut self) -> Option<&mut Modal> {
        self.items.front_mut()
    }

    pub fn pop(&mut self) -> Option<Modal> {
        self.version += 1;
        self.items.pop_front()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Modal> {
        self.items.iter_mut()
    }

    pub fn any(&self, matches: impl Fn(&Modal) -> bool) -> bool {
        self.items.iter().any(matches)
    }

    pub fn remove_where(&mut self, matches: impl Fn(&Modal) -> bool) -> bool {
        let Some(index) = self.items.iter().position(matches) else {
            return false;
        };
        self.items.remove(index);
        if index == 0 {
            self.version += 1;
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn version(&self) -> u64 {
        self.version
    }
}

const EDIT_QUIT_TEXT: &str = "An edit is still being saved to the server. Quit anyway? The remote file may be left incomplete; your copy stays in the state folder.";
const TRANSFER_QUIT_TEXT: &str = porthmos_core::transfer::QUIT_WITH_TRANSFERS_TEXT;

pub fn quit_text(transfers: bool, edits: bool) -> String {
    match (transfers, edits) {
        (true, true) => format!("{EDIT_QUIT_TEXT}\n\n{TRANSFER_QUIT_TEXT}"),
        (false, true) => EDIT_QUIT_TEXT.to_string(),
        _ => TRANSFER_QUIT_TEXT.to_string(),
    }
}

pub fn edit_question_title(name: &str, kind: EditQuestionKind) -> String {
    let name = fit(&printable(name), NAME_CELLS);
    match kind {
        EditQuestionKind::Upload => format!("Upload your changes to {name}?"),
        EditQuestionKind::Conflict => format!("{name} changed on the server while you were editing"),
    }
}

pub fn delete_summary(entries: &[Entry], place: &str) -> Vec<String> {
    porthmos_core::delete_prompt::delete_summary(entries, place, |name| fit(name, NAME_CELLS))
}

pub fn delete_lines(entries: &[Entry], place: &str, count: Option<&DeleteCount>) -> Vec<String> {
    let mut lines = delete_summary(entries, place);
    lines.extend(count.map(|count| contents_line(&count.contents)));
    lines
}

#[cfg(test)]
mod tests;

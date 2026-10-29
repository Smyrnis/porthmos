use std::collections::VecDeque;

use porthmos_core::edit::{EditQuestionKind, printable};
use porthmos_core::{Entry, Location, VfsPath};

use crate::app::PanelSide;
use crate::conflicts::ConflictView;
use crate::dialogs::Dialog;
use crate::view::text_fit::fit;

pub const MAX_LISTED_NAMES: usize = 5;
const NAME_CELLS: usize = 60;

#[derive(Debug, Clone)]
pub enum Modal {
    Question(Dialog),
    NewFolder { side: PanelSide, text: String, error: Option<String> },
    Rename { side: PanelSide, original: Entry, text: String, error: Option<String> },
    BookmarkName { location: Location, path: VfsPath, text: String, error: Option<String> },
    Info { title: String, body: String },
    ConfirmClearHistory,
    ConfirmDelete { side: PanelSide, entries: Vec<Entry> },
    Conflict { view: ConflictView, apply_to_rest: bool },
    EditQuestion { edit_id: u64, name: String, kind: EditQuestionKind },
    EditWaiting { edit_id: u64, name: String },
    ConfirmQuit { transfers: bool, edits: bool },
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
const TRANSFER_QUIT_TEXT: &str =
    "Some transfers are still running or queued. Quit anyway? Unfinished transfers can be resumed next time.";

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

pub fn delete_summary(entries: &[Entry]) -> Vec<String> {
    let mut lines = Vec::new();
    match entries {
        [only] => lines.push(format!("Delete {}?", fit(&only.name, NAME_CELLS))),
        _ => {
            lines.push(format!("Delete {} items?", entries.len()));
            for entry in entries.iter().take(MAX_LISTED_NAMES) {
                let name = if entry.is_dir { format!("{}/", entry.name) } else { entry.name.clone() };
                lines.push(fit(&name, NAME_CELLS));
            }
            if entries.len() > MAX_LISTED_NAMES {
                lines.push(format!("and {} more", entries.len() - MAX_LISTED_NAMES));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests;

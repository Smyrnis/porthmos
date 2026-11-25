pub mod properties;

use std::collections::VecDeque;
use std::sync::Arc;

use porthmos_core::delete_prompt::count::{Contents, contents_line};
use porthmos_core::diff::DiffOutcome;
use porthmos_core::edit::{EditQuestionKind, printable};
use porthmos_core::sync::{SyncAction, SyncBy, SyncDirection};
use porthmos_core::{Entry, Location, Message, SessionId, TunnelKey, VfsPath};

use crate::app::PanelSide;
use crate::conflicts::ConflictView;
use crate::dialogs::Dialog;
use crate::tunnel_form::{DraftError, Field, TunnelDraft};
use crate::view::text_fit::fit;

const NAME_CELLS: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteCount {
    pub request_id: u64,
    pub contents: Contents,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DiffPane {
    #[default]
    Idle,
    Comparing,
    Ready(Arc<DiffOutcome>),
    Failed(Message),
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
    Permissions {
        side: PanelSide,
        entries: Vec<Entry>,
        mode: u32,
        recursive: bool,
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
    Properties {
        location: Location,
        request_id: u64,
        pane: properties::PropertiesPane,
    },
    ConfirmClearHistory,
    OpenLink {
        target: String,
    },
    ConfirmPaste {
        terminal: porthmos_core::TerminalId,
        text: String,
    },
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
        diff: DiffPane,
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
        mirror: bool,
        error: Option<String>,
    },
    ConfirmSyncDelete {
        sync_id: u64,
        choices: Vec<(u32, SyncAction)>,
        message: String,
    },
    Tunnel {
        draft: Box<TunnelDraft>,
        connections: Vec<String>,
        errors: Vec<(Field, DraftError)>,
    },
    ConfirmRestartTunnel {
        key: TunnelKey,
    },
    ConfirmDeleteTunnel {
        key: TunnelKey,
    },
}

impl Modal {
    pub fn is_risky_to_accept(&self) -> bool {
        matches!(
            self,
            Modal::Question(Dialog::Confirm { risky: true, .. })
                | Modal::ConfirmDelete { .. }
                | Modal::ConfirmSyncDelete { .. }
                | Modal::ConfirmClearHistory
                | Modal::OpenLink { .. }
                | Modal::ConfirmQuit { .. }
                | Modal::ConfirmDeleteTunnel { .. }
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

pub fn quit_text(transfers: bool, edits: bool) -> String {
    let edit_text = porthmos_core::i18n::text("edit.quit_question");
    let transfer_text = porthmos_core::transfer::quit_with_transfers_text();
    match (transfers, edits) {
        (true, true) => format!("{edit_text}\n\n{transfer_text}"),
        (false, true) => edit_text,
        _ => transfer_text,
    }
}

pub fn edit_question_title(name: &str, kind: EditQuestionKind) -> String {
    let name = fit(&printable(name), NAME_CELLS);
    match kind {
        EditQuestionKind::Upload => porthmos_core::i18n::text_with("edit.upload_question", &[("name", &name)]),
        EditQuestionKind::Conflict => porthmos_core::i18n::text_with("edit.conflict_title", &[("name", &name)]),
    }
}

pub fn properties_title(entries: &[Entry]) -> String {
    match entries {
        [only] => porthmos_core::i18n::text_with(
            "properties.title_one",
            &[("name", &fit(&printable(&only.name), NAME_CELLS))],
        ),
        many => porthmos_core::i18n::text_with("properties.title_many", &[("count", &many.len().to_string())]),
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

use chrono::Local;
use porthmos_core::history::{self, HistoryEntry};
use porthmos_core::i18n;

use super::*;
use crate::widgets::{file_list::format_size, filter_line::FilterLine, paging::Paged};

pub(super) fn details_title(entry: &HistoryEntry) -> String {
    details(entry).0
}

pub(super) fn details_text(entry: &HistoryEntry) -> String {
    details(entry).1
}

fn details(entry: &HistoryEntry) -> (String, String) {
    let finished = entry.finished_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string();
    history::details(entry, &finished, &format_size(entry.bytes, false))
}

impl App {
    pub(super) fn open_history_screen(&mut self) {
        self.screen = Screen::History;
        self.history.go_to_top();
        self.core.send(Command::ListHistory);
    }

    pub(super) fn apply_history_action(&mut self, action: Action) {
        match action {
            Action::Up => self.history.move_cursor(-1),
            Action::Down => self.history.move_cursor(1),
            Action::PageUp => self.history.page_up(),
            Action::PageDown => self.history.page_down(),
            Action::First => self.history.cursor_to_first(),
            Action::Last => self.history.cursor_to_last(),
            Action::Open => self.open_history_details(),
            _ => {}
        }
    }

    fn open_history_details(&mut self) {
        let Some(entry) = self.history.selected() else {
            return;
        };
        let dialog = MessageDialog::new(details_title(entry), details_text(entry));
        self.dialog = Some(Dialog::Message(dialog));
    }

    pub(super) fn open_clear_history_dialog(&mut self) {
        if self.history.total() == 0 {
            return;
        }
        self.dialog = Some(Dialog::Confirm(ConfirmDialog::new(i18n::text("history.clear_confirm"))));
        self.pending_action = Some(PendingAction::ClearHistory);
    }
}

#[cfg(test)]
mod tests;

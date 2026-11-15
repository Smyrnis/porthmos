use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use iced::widget::operation;
use porthmos_core::edit::printable;
use porthmos_core::import::{ImportPlan, ImportSource, path_from_text};
use porthmos_core::{Command, Environment, Severity};

use super::{App, Message};
use crate::connections::Screen;
use crate::import_view::{ImportSourceState, ImportView, default_path_text, pick_request};

async fn pick(source: ImportSource, current_path: String, folder: bool) -> Option<PathBuf> {
    let request = pick_request(source, &current_path);
    let mut dialog = rfd::AsyncFileDialog::new();
    if let Some(directory) = request.start_dir {
        dialog = dialog.set_directory(directory);
    }
    let handle = if folder {
        dialog.pick_folder().await
    } else {
        dialog.add_filter(request.filter_name, request.extensions).add_filter("All files", &["*"]).pick_file().await
    };
    handle.map(|handle| handle.path().to_path_buf())
}

impl App {
    pub fn import_input_id() -> iced::widget::Id {
        App::form_input_id("import-path")
    }

    pub(in crate::app) fn import_pressed(&mut self) -> Task<Message> {
        if !matches!(self.connections_screen, Some(Screen::List)) {
            return Task::none();
        }
        self.set_screen(Some(Screen::ImportSource(ImportSourceState::new(ImportSource::ALL[0]))));
        operation::focus(App::import_input_id())
    }

    fn import_source_state(&mut self) -> Option<&mut ImportSourceState> {
        match self.connections_screen.as_mut() {
            Some(Screen::ImportSource(state)) => Some(state),
            _ => None,
        }
    }

    pub(in crate::app) fn import_source_chosen(&mut self, chosen: ImportSource) {
        let Some(state) = self.import_source_state().filter(|state| state.waiting.is_none()) else {
            return;
        };
        state.source = chosen;
        state.error = None;
        if state.path.is_empty() || state.path == state.prefilled {
            state.prefilled = default_path_text(chosen);
            state.path.clone_from(&state.prefilled);
        }
    }

    pub(in crate::app) fn import_path_typed(&mut self, text: String) {
        if let Some(state) = self.import_source_state().filter(|state| state.waiting.is_none()) {
            state.path = text;
            state.error = None;
        }
    }

    pub(in crate::app) fn import_browse(&mut self, folder: bool) -> Task<Message> {
        let Some(state) = self.import_source_state().filter(|state| state.waiting.is_none()) else {
            return Task::none();
        };
        let (source, current) = (state.source, state.path.clone());
        Task::perform(pick(source, current, folder), Message::ImportPathPicked)
    }

    pub(in crate::app) fn import_path_picked(&mut self, picked: Option<PathBuf>) {
        let Some(picked) = picked else {
            return;
        };
        self.import_path_typed(picked.to_string_lossy().into_owned());
    }

    pub(in crate::app) fn import_read_pressed(&mut self) {
        let import_id = self.next_import_id + 1;
        let Some(state) = self.import_source_state().filter(|state| state.waiting.is_none()) else {
            return;
        };
        if state.path.trim().is_empty() {
            state.error = Some(state.source.missing_path_message().to_string());
            return;
        }
        let command = Command::ReadImport {
            import_id,
            source: state.source,
            path: path_from_text(&state.path, Environment::from_process().home.as_deref()),
        };
        state.waiting = Some(import_id);
        state.error = None;
        self.next_import_id = import_id;
        self.core.send(command);
    }

    pub(in crate::app) fn import_plan_arrived(&mut self, plan: Arc<ImportPlan>) {
        let import_id = plan.import_id;
        let Some(state) = self.import_source_state().filter(|state| state.waiting == Some(import_id)) else {
            self.core.send(Command::CancelImport { import_id });
            return;
        };
        if plan.rows.is_empty() {
            state.error = Some(format!("No connections found in that {} source", plan.source.label()));
            state.waiting = None;
            self.core.send(Command::CancelImport { import_id });
            return;
        }
        self.set_screen(Some(Screen::ImportReview(ImportView::new(&plan))));
    }

    pub(in crate::app) fn import_read_failed(&mut self, message: &porthmos_core::Message) {
        if let Some(state) = self.import_source_state().filter(|state| state.waiting.is_some()) {
            state.waiting = None;
            state.error = Some(printable(&message.one_line()));
        }
    }

    fn import_view_mut(&mut self) -> Option<&mut ImportView> {
        match self.connections_screen.as_mut() {
            Some(Screen::ImportReview(view)) => Some(view),
            _ => None,
        }
    }

    pub(in crate::app) fn import_row_toggled(&mut self, index: usize) {
        if let Some(view) = self.import_view_mut() {
            view.toggle(index);
        }
    }

    pub(in crate::app) fn import_scrolled(&mut self, offset: f32) {
        if let Some(view) = self.import_view_mut() {
            view.set_scroll(offset);
        }
    }

    pub(in crate::app) fn import_tick_new(&mut self) {
        if let Some(view) = self.import_view_mut() {
            view.tick_new();
        }
    }

    pub(in crate::app) fn import_untick_all(&mut self) {
        if let Some(view) = self.import_view_mut() {
            view.untick_all();
        }
    }

    pub(in crate::app) fn import_run_pressed(&mut self) {
        let Some(view) = self.import_view_mut() else {
            return;
        };
        let rows = view.ticked_rows();
        let import_id = view.import_id();
        if rows.is_empty() {
            return;
        }
        self.connections_screen = None;
        self.set_screen(Some(Screen::List));
        self.core.send(Command::RunImport { import_id, rows });
    }

    pub(in crate::app) fn import_cancelled(&mut self) {
        if matches!(self.connections_screen, Some(Screen::ImportReview(_))) {
            self.set_screen(Some(Screen::List));
        }
    }

    pub(in crate::app) fn import_done(&mut self, added: usize, skipped: usize) {
        let plural = if added == 1 { "" } else { "s" };
        let mut message = format!("Imported {added} connection{plural}");
        if skipped > 0 {
            message.push_str(&format!(" ({skipped} skipped)"));
        }
        self.push_notice(Severity::Info, message);
    }
}

#[cfg(test)]
mod tests;

use std::sync::Arc;

use porthmos_core::{
    Command, Environment, Severity,
    import::{ImportPlan, ImportSource, default_location_here, path_from_text},
};

use super::{App, PendingAction};
use crate::widgets::dialog::{
    Dialog, FormDialog, FormField,
    import::{ImportChoice, ImportDialog},
};

const SOURCE: &str = "source";
const PATH: &str = "path";

#[derive(Default)]
pub(super) struct ImportState {
    last_id: u64,
    waiting: Option<u64>,
    prefilled_path: String,
}

fn source_value(source: ImportSource) -> &'static str {
    match source {
        ImportSource::FileZilla => "filezilla",
        ImportSource::WinScp => "winscp",
        ImportSource::Putty => "putty",
    }
}

fn source_from_value(value: &str) -> Option<ImportSource> {
    ImportSource::ALL.into_iter().find(|source| source_value(*source) == value)
}

fn default_path_text(source: ImportSource) -> String {
    default_location_here(source).map(|path| path.to_string_lossy().into_owned()).unwrap_or_default()
}

fn selected_source(form: &FormDialog) -> Option<ImportSource> {
    source_from_value(&form.value(SOURCE)?)
}

impl App {
    pub(super) fn open_import_dialog(&mut self) {
        let choices = ImportSource::ALL
            .into_iter()
            .map(|source| (source_value(source).to_string(), source.label().to_string()))
            .collect();
        let first = ImportSource::ALL[0];
        let prefilled = default_path_text(first);
        let fields = vec![
            FormField::choice(SOURCE, "Import from", choices, source_value(first)),
            FormField::text(PATH, "Path", prefilled.clone()),
        ];
        let mut form = FormDialog::new("Import connections", fields);
        form.hint = Some(first.hint().to_string());
        self.import.prefilled_path = prefilled;
        self.dialog = Some(Dialog::Form(form));
        self.pending_action = Some(PendingAction::ImportSource);
    }

    pub(super) fn import_source_changed(&mut self) {
        let Some(Dialog::Form(form)) = self.dialog.as_mut() else {
            return;
        };
        let Some(source) = selected_source(form) else {
            return;
        };
        form.hint = Some(source.hint().to_string());
        let untouched = form.value(PATH).is_some_and(|path| path.is_empty() || path == self.import.prefilled_path);
        if untouched {
            let replacement = default_path_text(source);
            if let Some(field) = form.fields.iter_mut().find(|field| field.key == PATH) {
                field.cursor = replacement.chars().count();
                field.value = replacement.clone();
            }
            self.import.prefilled_path = replacement;
        }
    }

    pub(super) fn submit_import_source(&mut self, values: Vec<(&'static str, String)>) {
        let value = |key: &str| values.iter().find(|(name, _)| *name == key).map(|(_, value)| value.as_str());
        let Some(source) = value(SOURCE).and_then(source_from_value) else {
            return;
        };
        let text = value(PATH).unwrap_or_default();
        if text.trim().is_empty() {
            self.set_form_error(Some(PATH), source.missing_path_message().to_string());
            return;
        }
        let path = path_from_text(text, Environment::from_process().home.as_deref());
        self.import.last_id += 1;
        let import_id = self.import.last_id;
        self.import.waiting = Some(import_id);
        self.dialog = None;
        self.pending_action = None;
        self.core.send(Command::ReadImport { import_id, source, path });
    }

    pub(super) fn apply_import_plan(&mut self, plan: Arc<ImportPlan>) {
        let import_id = plan.import_id;
        if self.import.waiting != Some(import_id) {
            self.core.send(Command::CancelImport { import_id });
            return;
        }
        self.import.waiting = None;
        if plan.rows.is_empty() {
            self.core.send(Command::CancelImport { import_id });
            self.notifications
                .push(Severity::Warning, format!("No connections found in that {} source", plan.source.label()));
            return;
        }
        if self.dialog.is_some() {
            self.core.send(Command::CancelImport { import_id });
            self.notifications.push(Severity::Warning, "The import was cancelled because another dialog was open");
            return;
        }
        self.dialog = Some(Dialog::Import(ImportDialog::new(&plan)));
        self.pending_action = Some(PendingAction::Import { import_id });
    }

    pub(super) fn apply_import_choice(&mut self, choice: ImportChoice) {
        let Some(&PendingAction::Import { import_id }) = self.pending_action.as_ref() else {
            return;
        };
        match choice {
            ImportChoice::Pending => {}
            ImportChoice::Run(rows) => {
                self.dialog = None;
                self.pending_action = None;
                self.core.send(Command::RunImport { import_id, rows });
            }
            ImportChoice::Cancel => {
                self.dialog = None;
                self.pending_action = None;
                self.core.send(Command::CancelImport { import_id });
            }
        }
    }

    pub(super) fn apply_import_done(&mut self, added: usize, skipped: usize) {
        let plural = if added == 1 { "" } else { "s" };
        let mut message = format!("Imported {added} connection{plural}");
        if skipped > 0 {
            message.push_str(&format!(" ({skipped} skipped)"));
        }
        self.notifications.push(Severity::Info, message);
    }
}

#[cfg(test)]
mod tests;

use porthmos_core::i18n::label as tr;
use std::path::PathBuf;

use porthmos_core::{
    Environment,
    edit::printable,
    import::{ImportPlan, ImportSource, Verdict, default_location_here, path_from_text},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSourceState {
    pub source: ImportSource,
    pub path: String,
    pub prefilled: String,
    pub error: Option<String>,
    pub waiting: Option<u64>,
}

pub fn default_path_text(source: ImportSource) -> String {
    default_location_here(source).map(|path| path.to_string_lossy().into_owned()).unwrap_or_default()
}

impl ImportSourceState {
    pub fn new(source: ImportSource) -> Self {
        let path = default_path_text(source);
        Self { source, prefilled: path.clone(), path, error: None, waiting: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickRequest {
    pub filter_name: String,
    pub extensions: &'static [&'static str],
    pub start_dir: Option<PathBuf>,
}

pub fn pick_request(source: ImportSource, current_path: &str) -> PickRequest {
    let (filter_name, extensions): (String, &'static [&'static str]) = match source {
        ImportSource::FileZilla => (tr("FileZilla site manager"), &["xml"]),
        ImportSource::WinScp => (tr("WinSCP settings"), &["ini"]),
        ImportSource::Putty => (tr("PuTTY registry export"), &["reg"]),
    };
    let typed = (!current_path.trim().is_empty())
        .then(|| path_from_text(current_path, Environment::from_process().home.as_deref()));
    let from_typed = typed.and_then(|path| if path.is_dir() { Some(path) } else { path.parent().map(PathBuf::from) });
    let from_default = || {
        default_location_here(source)
            .and_then(|path| if path.is_dir() { Some(path) } else { path.parent().map(PathBuf::from) })
    };
    let start_dir = from_typed.filter(|dir| dir.is_dir()).or_else(from_default);
    PickRequest { filter_name, extensions, start_dir }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportViewRow {
    pub ticked: bool,
    pub tickable: bool,
    pub invalid: bool,
    pub name: String,
    pub summary: String,
    pub group: String,
    pub verdict_text: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImportView {
    import_id: u64,
    source: ImportSource,
    rows: Vec<ImportViewRow>,
    verdicts: Vec<Verdict>,
    scroll: f32,
}

impl ImportView {
    pub fn new(plan: &ImportPlan) -> Self {
        let rows = plan
            .rows
            .iter()
            .map(|row| {
                let tickable = row.verdict == Verdict::New;
                ImportViewRow {
                    ticked: tickable,
                    tickable,
                    invalid: matches!(row.verdict, Verdict::Invalid(_)),
                    name: printable(&row.name),
                    summary: printable(&row.summary()),
                    group: row
                        .candidate
                        .as_ref()
                        .and_then(|candidate| candidate.group.as_deref())
                        .map(printable)
                        .unwrap_or_default(),
                    verdict_text: printable(&row.status().unwrap_or_default()),
                    notes: row
                        .candidate
                        .as_ref()
                        .map(|candidate| candidate.notes.iter().map(|note| printable(note)).collect())
                        .unwrap_or_default(),
                }
            })
            .collect();
        let verdicts = plan.rows.iter().map(|row| row.verdict.clone()).collect();
        Self { import_id: plan.import_id, source: plan.source, rows, verdicts, scroll: 0.0 }
    }

    pub fn import_id(&self) -> u64 {
        self.import_id
    }

    pub fn source(&self) -> ImportSource {
        self.source
    }

    pub fn rows(&self) -> &[ImportViewRow] {
        &self.rows
    }

    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    pub fn set_scroll(&mut self, offset: f32) {
        self.scroll = offset.max(0.0);
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(row) = self.rows.get_mut(index).filter(|row| row.tickable) {
            row.ticked = !row.ticked;
        }
    }

    pub fn tick_new(&mut self) {
        self.set_all(true);
    }

    pub fn untick_all(&mut self) {
        self.set_all(false);
    }

    fn set_all(&mut self, ticked: bool) {
        for row in self.rows.iter_mut().filter(|row| row.tickable) {
            row.ticked = ticked;
        }
    }

    pub fn ticked_rows(&self) -> Vec<u32> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.ticked)
            .map(|(index, _)| u32::try_from(index).unwrap_or(u32::MAX))
            .collect()
    }

    pub fn summary(&self) -> String {
        let count = |wanted: fn(&Verdict) -> bool| self.verdicts.iter().filter(|verdict| wanted(verdict)).count();
        let parts = [
            (count(|verdict| matches!(verdict, Verdict::New)), tr("new")),
            (count(|verdict| matches!(verdict, Verdict::Exists)), tr("existing")),
            (count(|verdict| matches!(verdict, Verdict::Unsupported(_))), tr("unsupported")),
            (count(|verdict| matches!(verdict, Verdict::Invalid(_))), tr("invalid")),
        ];
        parts
            .iter()
            .filter(|(number, _)| *number > 0)
            .map(|(number, word)| format!("{number} {word}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests;

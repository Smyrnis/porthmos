pub(crate) mod clean;
pub(crate) mod escape;
pub(crate) mod filezilla;
mod finish;
mod location;
pub(crate) mod putty;
mod read;
#[cfg(test)]
pub(crate) mod testing;
pub(crate) mod winscp;

pub use location::{default_location, default_location_here, path_from_text};
pub use read::read_plan;

use std::{collections::BTreeMap, fmt};

use crate::i18n;

pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

pub fn done_message(added: usize, skipped: usize) -> String {
    let mut message = i18n::plural("import.done", added);
    if skipped > 0 {
        message.push_str(&i18n::text_with("import.skipped", &[("skipped", &skipped.to_string())]));
    }
    message
}

pub fn nothing_found_message(source: ImportSource) -> String {
    i18n::text_with("import.nothing_found", &[("source", source.label())])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSource {
    FileZilla,
    WinScp,
    Putty,
}

impl ImportSource {
    pub const ALL: [ImportSource; 3] = [ImportSource::FileZilla, ImportSource::WinScp, ImportSource::Putty];

    pub fn label(self) -> &'static str {
        match self {
            ImportSource::FileZilla => "FileZilla",
            ImportSource::WinScp => "WinSCP",
            ImportSource::Putty => "PuTTY",
        }
    }

    pub fn hint(self) -> String {
        let key = match self {
            ImportSource::FileZilla => "import.hint.filezilla",
            ImportSource::WinScp => "import.hint.winscp",
            ImportSource::Putty => "import.hint.putty",
        };
        i18n::text(key)
    }

    pub fn missing_path_message(self) -> String {
        let key = match self {
            ImportSource::Putty => "import.path_putty",
            ImportSource::FileZilla | ImportSource::WinScp => "import.path_file",
        };
        i18n::text(key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub group: Option<String>,
    pub options: BTreeMap<String, String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    New,
    Exists,
    Unsupported(String),
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRow {
    pub name: String,
    pub candidate: Option<Candidate>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub import_id: u64,
    pub source: ImportSource,
    pub rows: Vec<ImportRow>,
}

impl ImportRow {
    pub fn summary(&self) -> String {
        let Some(candidate) = &self.candidate else {
            return String::new();
        };
        let port = candidate.port.map(|port| format!(":{port}")).unwrap_or_default();
        if candidate.username.is_empty() {
            format!("{} {}{port}", candidate.protocol, candidate.host)
        } else {
            format!("{} {}@{}{port}", candidate.protocol, candidate.username, candidate.host)
        }
    }

    pub fn status(&self) -> Option<String> {
        match &self.verdict {
            Verdict::New => None,
            Verdict::Exists => Some(i18n::text("import.exists")),
            Verdict::Unsupported(reason) => Some(reason.clone()),
            Verdict::Invalid(reason) => Some(i18n::text_with("import.cannot", &[("reason", reason)])),
        }
    }

    pub fn notes_line(&self) -> Option<String> {
        let notes = &self.candidate.as_ref()?.notes;
        (!notes.is_empty()).then(|| notes.join("; "))
    }
}

impl ImportPlan {
    pub fn new_count(&self) -> usize {
        self.rows.iter().filter(|row| row.verdict == Verdict::New).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    TooLarge,
    Unreadable(String),
    WrongFormat(&'static str),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::TooLarge => f.write_str(&i18n::text_with(
                "import.too_large",
                &[("limit", &(MAX_INPUT_BYTES / (1024 * 1024)).to_string())],
            )),
            ImportError::Unreadable(cause) => f.write_str(&i18n::text_with("import.unreadable", &[("cause", cause)])),
            ImportError::WrongFormat(key) => f.write_str(&i18n::text(key)),
        }
    }
}

impl std::error::Error for ImportError {}

#[cfg(test)]
mod tests;

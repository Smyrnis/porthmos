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

pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

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

    pub fn hint(self) -> &'static str {
        match self {
            ImportSource::FileZilla => "FileZilla: the sitemanager.xml file. Passwords are not imported.",
            ImportSource::WinScp => "WinSCP: the WinSCP.ini file. Passwords are not imported.",
            ImportSource::Putty => "PuTTY: the sessions folder, or a .reg export. Passwords are not imported.",
        }
    }

    pub fn missing_path_message(self) -> &'static str {
        match self {
            ImportSource::Putty => "Enter the path of the sessions folder or .reg file",
            ImportSource::FileZilla | ImportSource::WinScp => "Enter the path of the file",
        }
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
            Verdict::Exists => Some("already exists".to_string()),
            Verdict::Unsupported(reason) => Some(reason.clone()),
            Verdict::Invalid(reason) => Some(format!("can't import: {reason}")),
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
            ImportError::TooLarge => write!(f, "The file is larger than {} MiB", MAX_INPUT_BYTES / (1024 * 1024)),
            ImportError::Unreadable(cause) => write!(f, "Can't read the file: {cause}"),
            ImportError::WrongFormat(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ImportError {}

#[cfg(test)]
mod tests;

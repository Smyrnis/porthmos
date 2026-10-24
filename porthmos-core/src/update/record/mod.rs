use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    Paths,
    persist::{Loaded, read_or_set_aside, toml_problem},
};

#[derive(Default, Deserialize, Serialize)]
struct RecordFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_checked: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_failure: Option<String>,
}

pub(crate) struct UpdateRecord {
    path: PathBuf,
    file: RecordFile,
    writable: bool,
}

impl UpdateRecord {
    pub(crate) fn load(paths: &Paths) -> (UpdateRecord, Option<String>) {
        let path = paths.update_state_file();
        let mut record = UpdateRecord { path: path.clone(), file: RecordFile::default(), writable: true };
        match read_or_set_aside(&path, "update state", |text| {
            toml::from_str::<RecordFile>(text).map_err(|err| toml_problem(text, &err))
        }) {
            Loaded::Missing => (record, None),
            Loaded::Ready(file) => {
                record.file = file;
                (record, None)
            }
            Loaded::SetAside { warning, protected } => {
                record.writable = !protected;
                (record, Some(warning))
            }
        }
    }

    pub(crate) fn last_checked(&self) -> Option<DateTime<Utc>> {
        self.file.last_checked
    }

    pub(crate) fn set_last_checked(&mut self, at: DateTime<Utc>) {
        self.file.last_checked = Some(at);
    }

    pub(crate) fn last_failure(&self) -> Option<&str> {
        self.file.last_failure.as_deref()
    }

    pub(crate) fn set_last_failure(&mut self, failure: Option<String>) {
        self.file.last_failure = failure;
    }

    pub(crate) fn is_writable(&self) -> bool {
        self.writable
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn save(&self) -> Result<()> {
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        crate::persist::write_atomic(&self.path, self.render()?.as_bytes(), 0o600)
    }

    pub(crate) fn render(&self) -> Result<String> {
        Ok(toml::to_string(&self.file)?)
    }
}

#[cfg(test)]
mod tests;

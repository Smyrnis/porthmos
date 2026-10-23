use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateDir(PathBuf);

impl UpdateDir {
    pub fn new(path: impl Into<PathBuf>) -> UpdateDir {
        UpdateDir(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    pub version: Version,
    pub binary: PathBuf,
    pub sha256: String,
    pub size: u64,
}

const RECORD_FILE: &str = "staged.toml";

#[derive(Serialize, Deserialize)]
struct Record {
    version: String,
    binary: String,
    sha256: String,
    size: u64,
}

fn is_plain_relative(path: &Path) -> bool {
    path.components().count() > 0 && path.components().all(|part| matches!(part, Component::Normal(_)))
}

fn read_record(dir: &UpdateDir) -> Option<Staged> {
    let text = fs::read_to_string(dir.path().join(RECORD_FILE)).ok()?;
    let record: Record = toml::from_str(&text).ok()?;
    let relative = Path::new(&record.binary);
    if !is_plain_relative(relative) {
        return None;
    }
    Some(Staged {
        version: record.version.parse().ok()?,
        binary: dir.path().join(relative),
        sha256: record.sha256,
        size: record.size,
    })
}

pub fn find_staged(dir: &UpdateDir, running: &Version) -> Option<Staged> {
    if !dir.path().join(RECORD_FILE).exists() {
        return None;
    }
    let usable = read_record(dir).filter(|staged| {
        staged.version > *running
            && fs::metadata(&staged.binary).is_ok_and(|meta| meta.is_file() && meta.len() == staged.size)
    });
    if usable.is_none() {
        discard_staged(dir);
    }
    usable
}

pub fn discard_staged(dir: &UpdateDir) {
    let _ = fs::remove_file(dir.path().join(RECORD_FILE));
    let Ok(entries) = fs::read_dir(dir.path()) else { return };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

pub(crate) fn record_staged(dir: &UpdateDir, staged: &Staged) -> io::Result<()> {
    let relative =
        staged.binary.strip_prefix(dir.path()).ok().filter(|path| is_plain_relative(path)).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "the staged binary is outside the update folder")
        })?;
    let record = Record {
        version: staged.version.to_string(),
        binary: relative.to_string_lossy().into_owned(),
        sha256: staged.sha256.clone(),
        size: staged.size,
    };
    let text =
        toml::to_string(&record).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    fs::create_dir_all(dir.path())?;
    let temporary = dir.path().join(format!("{RECORD_FILE}.tmp"));
    let written = fs::write(&temporary, text).and_then(|()| fs::rename(&temporary, dir.path().join(RECORD_FILE)));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

#[cfg(test)]
mod tests;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};

use porthmos_vfs::VfsPath;
use serde::{Deserialize, Serialize};

use crate::{
    Paths,
    persist::{Loaded, read_or_set_aside, toml_problem},
};

#[derive(Default, Deserialize)]
struct LastUsedFile {
    #[serde(default)]
    last_used: BTreeMap<String, DateTime<Utc>>,
    #[serde(default)]
    last_folder: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct LastUsedFileRef<'a> {
    last_used: &'a BTreeMap<String, DateTime<Utc>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    last_folder: &'a BTreeMap<String, String>,
}

pub struct LastUsed {
    path: PathBuf,
    times: BTreeMap<String, DateTime<Utc>>,
    folders: BTreeMap<String, String>,
    writable: bool,
}

impl LastUsed {
    pub fn load(paths: &Paths) -> (Self, Option<String>) {
        let path = paths.last_used_file();
        let mut last_used =
            Self { path: path.clone(), times: BTreeMap::new(), folders: BTreeMap::new(), writable: true };
        match read_or_set_aside(&path, "last-used times", |text| {
            toml::from_str::<LastUsedFile>(text).map_err(|err| toml_problem(text, &err))
        }) {
            Loaded::Missing => (last_used, None),
            Loaded::Ready(file) => {
                last_used.times = file.last_used;
                last_used.folders = file.last_folder;
                (last_used, None)
            }
            Loaded::SetAside { warning, protected } => {
                last_used.writable = !protected;
                (last_used, Some(warning))
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<DateTime<Utc>> {
        self.times.get(name).copied()
    }

    pub fn times(&self) -> BTreeMap<String, DateTime<Utc>> {
        self.times.clone()
    }

    pub fn record(&mut self, name: &str, at: DateTime<Utc>) {
        self.times.insert(name.to_string(), at);
    }

    pub fn folder(&self, name: &str) -> Option<VfsPath> {
        self.folders.get(name).map(|folder| VfsPath::new(folder.as_str()))
    }

    pub fn record_folder(&mut self, name: &str, folder: &VfsPath) -> bool {
        let text = folder.to_string_lossy();
        if self.folders.get(name).is_some_and(|known| *known == *text) {
            return false;
        }
        self.folders.insert(name.to_string(), text.into_owned());
        true
    }

    pub fn rename(&mut self, from: &str, to: &str) -> bool {
        if from == to {
            return false;
        }
        let carried_time = self.times.remove(from);
        let replaced_time = match carried_time {
            Some(at) => self.times.insert(to.to_string(), at),
            None => self.times.remove(to),
        };
        let carried_folder = self.folders.remove(from);
        let replaced_folder = match carried_folder.clone() {
            Some(folder) => self.folders.insert(to.to_string(), folder),
            None => self.folders.remove(to),
        };
        carried_time.is_some() || replaced_time.is_some() || carried_folder.is_some() || replaced_folder.is_some()
    }

    pub fn merge(&mut self, from: &str, to: &str) -> bool {
        if from == to {
            return false;
        }
        let time = self.times.remove(from);
        let folder = self.folders.remove(from);
        if time.is_none() && folder.is_none() {
            return false;
        }
        let existing = self.times.get(to).copied();
        let from_is_later = match (time, existing) {
            (Some(at), Some(existing)) => at > existing,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if let Some(at) = time {
            self.times.insert(to.to_string(), existing.map_or(at, |existing| existing.max(at)));
        }
        if let Some(folder) = folder
            && (from_is_later || !self.folders.contains_key(to))
        {
            self.folders.insert(to.to_string(), folder);
        }
        true
    }

    pub fn forget(&mut self, name: &str) -> bool {
        let time = self.times.remove(name).is_some();
        let folder = self.folders.remove(name).is_some();
        time || folder
    }

    pub fn render(&self) -> Result<String> {
        Ok(toml::to_string(&LastUsedFileRef { last_used: &self.times, last_folder: &self.folders })?)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }
}

#[cfg(test)]
mod tests;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

use crate::{
    Paths,
    persist::{Loaded, read_or_set_aside, toml_problem},
};

#[derive(Default, Deserialize)]
struct LastUsedFile {
    #[serde(default)]
    last_used: BTreeMap<String, DateTime<Utc>>,
}

#[derive(Serialize)]
struct LastUsedFileRef<'a> {
    last_used: &'a BTreeMap<String, DateTime<Utc>>,
}

pub struct LastUsed {
    path: PathBuf,
    times: BTreeMap<String, DateTime<Utc>>,
    writable: bool,
}

impl LastUsed {
    pub fn load(paths: &Paths) -> (Self, Option<String>) {
        let path = paths.last_used_file();
        let mut last_used = Self { path: path.clone(), times: BTreeMap::new(), writable: true };
        match read_or_set_aside(&path, "last-used times", |text| {
            toml::from_str::<LastUsedFile>(text).map_err(|err| toml_problem(text, &err))
        }) {
            Loaded::Missing => (last_used, None),
            Loaded::Ready(file) => {
                last_used.times = file.last_used;
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

    pub fn rename(&mut self, from: &str, to: &str) -> bool {
        if from == to {
            return false;
        }
        let carried = self.times.remove(from);
        let replaced = match carried {
            Some(at) => self.times.insert(to.to_string(), at),
            None => self.times.remove(to),
        };
        carried.is_some() || replaced.is_some()
    }

    pub fn merge(&mut self, from: &str, to: &str) -> bool {
        if from == to {
            return false;
        }
        let Some(at) = self.times.remove(from) else {
            return false;
        };
        let kept = self.times.get(to).map_or(at, |existing| (*existing).max(at));
        self.times.insert(to.to_string(), kept);
        true
    }

    pub fn forget(&mut self, name: &str) -> bool {
        self.times.remove(name).is_some()
    }

    pub fn render(&self) -> Result<String> {
        Ok(toml::to_string(&LastUsedFileRef { last_used: &self.times })?)
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

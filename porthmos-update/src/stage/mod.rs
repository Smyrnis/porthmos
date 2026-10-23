use std::{
    fmt, fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use anyhow::Result;
use semver::Version;

use crate::{
    Staged, UpdateDir, UpdateInfo,
    fetch::{FetchError, Fetcher},
    find_staged, sha256_hex_of_file,
    staged::{is_another_process_at_work, record_staged},
    unpack::{extract_binary, health_check},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageError {
    Download(String),
    Unpack(String),
    HealthCheck(String),
    Disk(String),
    Cancelled,
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StageError::Download(text) => write!(f, "couldn't download the update: {text}"),
            StageError::Unpack(text) => write!(f, "couldn't unpack the update: {text}"),
            StageError::HealthCheck(text) => write!(f, "the downloaded update does not run: {text}"),
            StageError::Disk(text) => write!(f, "couldn't store the update: {text}"),
            StageError::Cancelled => f.write_str("staging the update was cancelled"),
        }
    }
}

impl std::error::Error for StageError {}

const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const MAX_BINARY_BYTES: u64 = 256 * 1024 * 1024;
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
const ARCHIVE_FILE: &str = "archive.tar.gz";

pub struct Stager {
    fetcher: Fetcher,
    dir: UpdateDir,
}

fn from_fetch(error: FetchError) -> StageError {
    match error {
        FetchError::Cancelled => StageError::Cancelled,
        other => StageError::Download(other.to_string()),
    }
}

fn disk(error: impl fmt::Display) -> StageError {
    StageError::Disk(error.to_string())
}

fn remove_folder(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

impl Stager {
    pub fn new(dir: UpdateDir, user_agent: &str) -> Result<Stager> {
        Stager::with_policy(dir, user_agent, true)
    }

    fn with_policy(dir: UpdateDir, user_agent: &str, https_only: bool) -> Result<Stager> {
        Ok(Stager { fetcher: Fetcher::new(user_agent, ARCHIVE_TIMEOUT, https_only)?, dir })
    }

    pub async fn stage(
        &self, update: &UpdateInfo, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Staged, StageError> {
        if let Some(staged) =
            find_staged(&self.dir, &Version::new(0, 0, 0)).filter(|staged| staged.version == update.version)
        {
            return Ok(staged);
        }
        let scratch = self.dir.path().join(format!("download.{}", std::process::id()));
        let staging = self.dir.path().join(format!("{}.staging-{}", update.version, std::process::id()));
        let outcome = self.run(update, cancel, progress, &scratch, &staging).await;
        remove_folder(&scratch);
        remove_folder(&staging);
        outcome
    }

    async fn run(
        &self, update: &UpdateInfo, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64), scratch: &Path,
        staging: &Path,
    ) -> Result<Staged, StageError> {
        remove_folder(staging);
        fs::create_dir_all(scratch).map_err(disk)?;
        fs::create_dir_all(staging).map_err(disk)?;
        let archive = scratch.join(ARCHIVE_FILE);
        let artifact = &update.artifact;
        self.fetcher
            .get_to_file(&artifact.url, &archive, artifact.size, &artifact.sha256, cancel, progress)
            .await
            .map_err(from_fetch)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(StageError::Cancelled);
        }

        let binary = staging.join("porthmos");
        let (unpack_from, unpack_to) = (archive, binary.clone());
        tokio::task::spawn_blocking(move || extract_binary(&unpack_from, &unpack_to, MAX_BINARY_BYTES))
            .await
            .map_err(|error| StageError::Unpack(error.to_string()))?
            .map_err(|error| StageError::Unpack(format!("{error:#}")))?;

        let (checked, expected) = (binary.clone(), update.version.clone());
        tokio::task::spawn_blocking(move || health_check(&checked, &expected, HEALTH_TIMEOUT))
            .await
            .map_err(|error| StageError::HealthCheck(error.to_string()))?
            .map_err(|error| StageError::HealthCheck(format!("{error:#}")))?;

        let sha256 = sha256_hex_of_file(&binary).map_err(disk)?;
        let size = fs::metadata(&binary).map_err(disk)?.len();
        let folder = self.dir.path().join(update.version.to_string());
        remove_folder(&folder);
        fs::rename(staging, &folder).map_err(disk)?;
        let staged = Staged { version: update.version.clone(), binary: folder.join("porthmos"), sha256, size };
        record_staged(&self.dir, &staged).map_err(disk)?;
        self.forget_other_versions(&folder);
        Ok(staged)
    }

    fn forget_other_versions(&self, keep: &Path) {
        let Ok(entries) = fs::read_dir(self.dir.path()) else { return };
        for entry in entries.flatten() {
            if entry.path() != keep
                && entry.file_type().is_ok_and(|kind| kind.is_dir())
                && !is_another_process_at_work(&entry.path())
            {
                remove_folder(&entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests;

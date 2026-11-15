use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{Direction, JobStatus, TransferQueue};
use crate::{
    Paths,
    persist::{Loaded, read_or_set_aside, toml_problem},
};

pub const INTERRUPTED_REASON: &str = "Porthmos was closed before this finished";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedJob {
    pub connection: String,
    pub direction: Direction,
    pub local_path: String,
    pub remote_path: String,
    pub display_name: String,
    pub total_bytes: u64,
    #[serde(default)]
    pub transferred_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
    #[serde(default)]
    pub remove_source: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

#[derive(Deserialize)]
struct SavedFile {
    #[serde(default)]
    jobs: Vec<SavedJob>,
}

#[derive(Serialize)]
struct SavedFileRef<'a> {
    jobs: &'a [SavedJob],
}

pub fn unfinished(queue: &TransferQueue, connection_of: impl Fn(u64) -> String) -> Vec<SavedJob> {
    queue
        .jobs()
        .filter(|job| matches!(job.status, JobStatus::Queued | JobStatus::InProgress | JobStatus::Failed(_)))
        .map(|job| SavedJob {
            connection: connection_of(job.session_id),
            direction: job.direction,
            local_path: job.local_path.to_string_lossy().into_owned(),
            remote_path: job.remote_path.clone(),
            display_name: job.display_name.clone(),
            total_bytes: job.total_bytes,
            transferred_bytes: job.transferred_bytes,
            modified: job.modified,
            remove_source: job.remove_source,
            batch: job.batch_id,
            label: job.batch_id.and_then(|batch| queue.batch_label(batch)).map(str::to_string),
            failure: match &job.status {
                JobStatus::Failed(message) => Some(message.clone()),
                _ => None,
            },
        })
        .collect()
}

pub fn render(jobs: &[SavedJob]) -> Result<String> {
    Ok(toml::to_string(&SavedFileRef { jobs })?)
}

pub fn load(paths: &Paths) -> Loaded<Vec<SavedJob>> {
    read_or_set_aside(&paths.unfinished_transfers_file(), "unfinished transfers", |text| {
        toml::from_str::<SavedFile>(text).map(|file| file.jobs).map_err(|err| toml_problem(text, &err))
    })
}

#[cfg(test)]
mod tests;

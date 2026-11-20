use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{Direction, JobStatus, TransferQueue};
use crate::{
    Message, Paths,
    persist::{Loaded, read_or_set_aside, toml_problem},
};

pub fn interrupted_reason() -> Message {
    Message::from(crate::i18n::text("transfer.interrupted"))
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedMove {
    pub batch: u64,
    pub connection: String,
    pub direction: Direction,
    pub roots: Vec<String>,
}

#[derive(Deserialize)]
struct SavedFile {
    #[serde(default)]
    jobs: Vec<SavedJob>,
    #[serde(default)]
    moves: Vec<SavedMove>,
}

#[derive(Serialize)]
struct SavedFileRef<'a> {
    jobs: &'a [SavedJob],
    moves: &'a [SavedMove],
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
                JobStatus::Failed(message) => Some(message.to_string()),
                _ => None,
            },
        })
        .collect()
}

pub fn render(jobs: &[SavedJob]) -> Result<String> {
    render_with_moves(jobs, &[])
}

pub fn render_with_moves(jobs: &[SavedJob], moves: &[SavedMove]) -> Result<String> {
    Ok(toml::to_string(&SavedFileRef { jobs, moves })?)
}

pub fn load_moves(paths: &Paths) -> Vec<SavedMove> {
    std::fs::read_to_string(paths.unfinished_transfers_file())
        .ok()
        .and_then(|text| toml::from_str::<SavedFile>(&text).ok())
        .map(|file| file.moves)
        .unwrap_or_default()
}

pub fn load(paths: &Paths) -> Loaded<Vec<SavedJob>> {
    read_or_set_aside(&paths.unfinished_transfers_file(), "unfinished transfers", |text| {
        toml::from_str::<SavedFile>(text).map(|file| file.jobs).map_err(|err| toml_problem(text, &err))
    })
}

#[cfg(test)]
mod tests;

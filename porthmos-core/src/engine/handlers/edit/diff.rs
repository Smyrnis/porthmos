use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use porthmos_vfs::{ErrorKind, FileSystem, Reader, VfsPath};
use tokio::io::AsyncReadExt;

use super::{EditEvent, Engine};
use crate::{
    Message,
    diff::{DiffOutcome, compute},
    edit::{EditQuestionKind, printable},
    engine::{Event, Internal},
    failure_report,
    tasks::Scope,
};

const READ_CHUNK: usize = 64 * 1024;
const CANCEL_POLL: std::time::Duration = std::time::Duration::from_millis(100);
const CANCELLED: &str = "Comparison cancelled";

pub(super) enum Loaded {
    Whole(Vec<u8>),
    TooLarge,
    Cancelled,
}

pub(super) async fn read_capped(reader: &mut Reader, limit: u64, cancel: &AtomicBool) -> std::io::Result<Loaded> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0u8; READ_CHUNK];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Loaded::Cancelled);
        }
        let Ok(read) = tokio::time::timeout(CANCEL_POLL, reader.read(&mut chunk)).await else {
            continue;
        };
        let read = read?;
        if read == 0 {
            return Ok(Loaded::Whole(bytes));
        }
        if bytes.len() as u64 + read as u64 > limit {
            return Ok(Loaded::TooLarge);
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

struct Comparison {
    fs: Arc<dyn FileSystem>,
    remote: VfsPath,
    local_copy: PathBuf,
    name: String,
    limit: u64,
}

async fn compare(job: Comparison, cancel: Arc<AtomicBool>) -> Result<DiffOutcome, Message> {
    let Comparison { fs, remote, local_copy, name, limit } = job;
    let failed = |err: &dyn std::fmt::Display| -> Result<DiffOutcome, Message> {
        Err(failure_report(format!("Comparison failed: {}", printable(&name)), err))
    };
    let metadata = match fs.stat(&remote).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(DiffOutcome::RemoteGone),
        Err(err) => return failed(&err),
    };
    if metadata.is_dir() {
        return failed(&"the server's file is now a folder");
    }
    let local_size = match std::fs::metadata(&local_copy) {
        Ok(local) => local.len(),
        Err(err) => return failed(&err),
    };
    if metadata.size > limit || local_size > limit {
        return Ok(DiffOutcome::TooLarge { limit });
    }
    let mut reader = match fs.open_read(&remote, 0).await {
        Ok(reader) => reader,
        Err(err) => return failed(&err),
    };
    let remote_bytes = match read_capped(&mut reader, limit, &cancel).await {
        Ok(Loaded::Whole(bytes)) => bytes,
        Ok(Loaded::TooLarge) => return Ok(DiffOutcome::TooLarge { limit }),
        Ok(Loaded::Cancelled) => return Err(Message::new(CANCELLED)),
        Err(err) => return failed(&err),
    };
    let mine = match tokio::fs::read(&local_copy).await {
        Ok(bytes) => bytes,
        Err(err) => return failed(&err),
    };
    match tokio::task::spawn_blocking(move || compute(remote_bytes, mine)).await {
        Ok(outcome) => Ok(outcome),
        Err(err) => failed(&err),
    }
}

impl Engine {
    pub(crate) fn show_edit_diff(&mut self, edit_id: u64) {
        let Some(session) = self.edit.sessions.get(&edit_id) else {
            return;
        };
        if session.pending != Some(EditQuestionKind::Conflict) || session.diffing {
            return;
        }
        let Some(local_copy) = session.temp.as_ref().map(|temp| temp.file.clone()) else {
            return;
        };
        let (location, remote, name) = (session.location, session.remote_path.clone(), session.name.clone());
        let Some(fs) = self.fs_for(location) else {
            let message = failure_report(format!("Comparison failed: {}", printable(&name)), &"session disconnected");
            self.emit(Event::EditDiff { edit_id, result: Err(message) });
            return;
        };
        if let Some(session) = self.edit.sessions.get_mut(&edit_id) {
            session.diffing = true;
        }
        let limit = self.edit.settings.compare_limit_bytes();
        let internal = self.internal.clone();
        let job = Comparison { fs, remote, local_copy, name, limit };
        let cancel = self.tasks.spawn("edit-diff", Scope::Edit(edit_id), move |cancel| async move {
            let result = compare(job, cancel).await;
            let _ = internal.send(Internal::Edit(EditEvent::Diffed { edit_id, result }));
        });
        if let Some(session) = self.edit.sessions.get_mut(&edit_id) {
            session.diff_cancel = Some(cancel);
        }
    }

    pub(super) fn finish_diff(&mut self, edit_id: u64, result: Result<DiffOutcome, Message>) {
        let Some(session) = self.edit.sessions.get_mut(&edit_id) else {
            return;
        };
        if !session.diffing || session.pending != Some(EditQuestionKind::Conflict) {
            return;
        }
        session.diffing = false;
        session.diff_cancel = None;
        self.emit(Event::EditDiff { edit_id, result: result.map(Arc::new) });
    }
}

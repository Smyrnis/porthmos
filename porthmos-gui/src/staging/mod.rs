use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use porthmos_core::transfer::TransferSnapshot;
use porthmos_core::transfer::rows::{QueueRow, RowKind, RowState, percent_of};
use porthmos_core::{Entry, SessionId, VfsPath, from_os_path};

pub const PREFIX: &str = "porthmos-drag-";
pub const STALE_AFTER: Duration = Duration::from_secs(24 * 3600);
pub const SLOW_AFTER: Duration = Duration::from_secs(15);
pub const SLOW_LABEL: &str = "Taking long — keep holding or Esc";

#[derive(Debug)]
pub struct Staging {
    root: PathBuf,
    expected: Vec<String>,
    known: HashSet<RowKind>,
}

impl Staging {
    pub fn create(parent: &Path) -> io::Result<Staging> {
        let mut builder = tempfile::Builder::new();
        builder.prefix(PREFIX);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        let root = builder.tempdir_in(parent)?.keep();
        Ok(Staging { root, expected: Vec::new(), known: HashSet::new() })
    }

    pub fn expecting(mut self, names: Vec<String>) -> Staging {
        self.expected = names;
        self
    }

    pub fn remember(&mut self, snapshot: &TransferSnapshot) {
        self.known = snapshot.rows.iter().map(|row| row.kind).collect();
    }

    #[cfg(test)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn destination(&self) -> VfsPath {
        from_os_path(&self.root)
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        self.expected.iter().map(|name| self.root.join(name)).collect()
    }

    pub fn own_rows<'a>(&self, snapshot: &'a TransferSnapshot) -> Vec<&'a QueueRow> {
        snapshot.rows.iter().filter(|row| !self.known.contains(&row.kind)).collect()
    }

    pub fn finished(&self, snapshot: &TransferSnapshot, seen_a_row: bool) -> bool {
        seen_a_row
            && self.own_rows(snapshot).iter().all(|row| row.is_finished())
            && self.expected.iter().all(|name| self.root.join(name).exists())
    }

    pub fn failure(&self, snapshot: &TransferSnapshot) -> Option<String> {
        self.own_rows(snapshot).into_iter().find_map(|row| match row.state {
            RowState::Failed | RowState::PartlyFailed(_) => Some(match &row.failure {
                Some(failure) => format!("{}: {}", failure.file, failure.message),
                None => format!("{} could not be copied", row.label),
            }),
            RowState::Cancelled => Some(format!("{} was cancelled", row.label)),
            _ => None,
        })
    }

    pub fn progress(&self, snapshot: &TransferSnapshot) -> Option<u8> {
        let rows = self.own_rows(snapshot);
        let total: u64 = rows.iter().map(|row| row.bytes_total).sum();
        let done: u64 = rows.iter().map(|row| row.bytes_done).sum();
        (total > 0).then(|| percent_of(done, total))
    }

    pub fn remove(self) {
        let _ = fs::remove_dir_all(&self.root);
    }

    pub fn remove_after(self, delay: Duration) {
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            self.remove();
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Preparing,
    Aborting,
    Handed,
}

#[derive(Debug)]
pub struct Job {
    pub staging: Staging,
    pub session: SessionId,
    pub stage: Stage,
    pub seen_a_row: bool,
    cancel_sent: HashSet<RowKind>,
    started: Instant,
    slow: bool,
    count: usize,
    only_files: bool,
}

impl Job {
    pub fn new(staging: Staging, session: SessionId, entries: &[Entry]) -> Job {
        Job {
            staging,
            session,
            stage: Stage::Preparing,
            seen_a_row: false,
            cancel_sent: HashSet::new(),
            started: Instant::now(),
            slow: false,
            count: entries.len(),
            only_files: entries.iter().all(|entry| !entry.is_dir),
        }
    }

    pub fn observe(&mut self, snapshot: &TransferSnapshot) {
        self.seen_a_row |= !self.staging.own_rows(snapshot).is_empty();
    }

    pub fn tick(&mut self, now: Instant) {
        self.slow = now.duration_since(self.started) >= SLOW_AFTER;
    }

    pub fn rows_to_cancel(&mut self, snapshot: &TransferSnapshot) -> Vec<RowKind> {
        let unfinished: Vec<RowKind> =
            self.staging.own_rows(snapshot).into_iter().filter(|row| !row.is_finished()).map(|row| row.kind).collect();
        unfinished.into_iter().filter(|kind| self.cancel_sent.insert(*kind)).collect()
    }

    pub fn wound_down(&self, snapshot: &TransferSnapshot) -> bool {
        self.seen_a_row && self.staging.own_rows(snapshot).iter().all(|row| row.is_finished())
    }

    pub fn label(&self, snapshot: &TransferSnapshot) -> String {
        if self.slow {
            return SLOW_LABEL.to_string();
        }
        let noun = match (self.only_files, self.count) {
            (true, 1) => "file",
            (true, _) => "files",
            (false, 1) => "item",
            (false, _) => "items",
        };
        let base = format!("Preparing {} {noun}…", self.count);
        match self.staging.progress(snapshot) {
            Some(percent) => format!("{base} {percent}%"),
            None => base,
        }
    }
}

pub fn remove_stale(parent: &Path, older_than: Duration) {
    let Ok(listing) = fs::read_dir(parent) else {
        return;
    };
    let Some(cutoff) = SystemTime::now().checked_sub(older_than) else {
        return;
    };
    for item in listing.flatten() {
        let named_like_ours = item.file_name().to_string_lossy().starts_with(PREFIX);
        let Ok(metadata) = item.metadata() else {
            continue;
        };
        if named_like_ours && metadata.is_dir() && metadata.modified().is_ok_and(|modified| modified < cutoff) {
            let _ = fs::remove_dir_all(item.path());
        }
    }
}

#[cfg(test)]
mod tests;

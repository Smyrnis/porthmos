use porthmos_core::VfsPath;
use porthmos_core::properties::{Digest, Measured, digest::matches_expected};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasurePane {
    Running(Measured),
    Done(Measured),
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashState {
    Idle,
    Running { done: u64, total: u64 },
    Done(Digest),
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHash {
    pub path: VfsPath,
    pub name: String,
    pub listed_total: Option<u64>,
    pub state: HashState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertiesPane {
    pub title: String,
    pub measure: MeasurePane,
    pub files: Vec<FileHash>,
    pub expected: String,
}

impl PropertiesPane {
    pub fn new(title: String, files: Vec<(VfsPath, String)>) -> Self {
        let files = files
            .into_iter()
            .map(|(path, name)| FileHash { path, name, listed_total: None, state: HashState::Idle })
            .collect();
        Self { title, measure: MeasurePane::Running(Measured::default()), files, expected: String::new() }
    }

    pub fn set_progress(&mut self, measured: Measured) {
        if self.measuring() {
            self.measure = MeasurePane::Running(measured);
        }
    }

    pub fn set_measured(&mut self, result: Result<Measured, String>) {
        if self.measure == MeasurePane::Cancelled {
            return;
        }
        self.measure = match result {
            Ok(measured) => MeasurePane::Done(measured),
            Err(message) => MeasurePane::Failed(message),
        };
    }

    fn running_file(&mut self, path: &VfsPath) -> Option<&mut FileHash> {
        self.files.iter_mut().find(|file| &file.path == path && matches!(file.state, HashState::Running { .. }))
    }

    pub fn set_checksum_progress(&mut self, path: &VfsPath, done: u64, total: u64) {
        if let Some(file) = self.running_file(path) {
            file.listed_total = Some(total);
            file.state = HashState::Running { done, total };
        }
    }

    pub fn set_checksum(&mut self, path: &VfsPath, result: Result<Digest, String>) {
        if let Some(file) = self.running_file(path) {
            file.state = match result {
                Ok(digest) => HashState::Done(digest),
                Err(message) => HashState::Failed(message),
            };
        }
    }

    pub fn measuring(&self) -> bool {
        matches!(self.measure, MeasurePane::Running(_))
    }

    pub fn hashing(&self) -> bool {
        self.files.iter().any(|file| matches!(file.state, HashState::Running { .. }))
    }

    pub fn running(&self) -> bool {
        self.measuring() || self.hashing()
    }

    pub fn cancel_running(&mut self) {
        if self.measuring() {
            self.measure = MeasurePane::Cancelled;
        }
        for file in &mut self.files {
            if matches!(file.state, HashState::Running { .. }) {
                file.state = HashState::Cancelled;
            }
        }
    }

    pub fn can_compute(&self) -> bool {
        !self.files.is_empty() && !self.hashing()
    }

    pub fn begin_checksums(&mut self) -> Option<Vec<VfsPath>> {
        if !self.can_compute() {
            return None;
        }
        let mut paths = Vec::with_capacity(self.files.len());
        for file in &mut self.files {
            file.state = HashState::Running { done: 0, total: file.listed_total.unwrap_or(0) };
            paths.push(file.path.clone());
        }
        Some(paths)
    }

    pub fn has_expected_field(&self) -> bool {
        self.files.len() == 1
    }

    pub fn verdict(&self) -> Option<bool> {
        let [file] = self.files.as_slice() else {
            return None;
        };
        let HashState::Done(digest) = &file.state else {
            return None;
        };
        if self.expected.trim().is_empty() {
            return None;
        }
        Some(matches_expected(&self.expected, &digest.hex))
    }

    pub fn digest_text(&self, index: usize) -> Option<String> {
        match &self.files.get(index)?.state {
            HashState::Done(digest) => Some(digest.hex.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;

use std::time::Duration;

use porthmos_vfs::{ErrorKind, FileSystem, VfsPath};

pub const TEST_LIMIT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartFolder {
    Opens(String),
    Missing(String),
    Unreadable { path: String, message: String },
    Unchecked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestReport {
    pub elapsed: Duration,
    pub server: Option<String>,
    pub start_folder: StartFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotTrusted {
    HostKey { key_type: String, fingerprint: String },
    Certificate { fingerprint: String, subject: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStage {
    Settings,
    Connect,
    Login,
    StartFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOutcome {
    Connected(TestReport),
    NotTrusted(NotTrusted),
    Failed { stage: TestStage, message: String },
    TimedOut(TestStage),
    Cancelled,
}

pub fn stage_of(kind: ErrorKind) -> Option<TestStage> {
    match kind {
        ErrorKind::Auth | ErrorKind::AuthRejected => Some(TestStage::Login),
        ErrorKind::Cancelled => None,
        ErrorKind::Connect
        | ErrorKind::SessionStart
        | ErrorKind::Other
        | ErrorKind::Unsupported
        | ErrorKind::NotFound
        | ErrorKind::PermissionDenied => Some(TestStage::Connect),
    }
}

pub async fn check_start_folder(fs: &dyn FileSystem, configured: Option<&str>) -> StartFolder {
    let path = match configured {
        Some(path) => VfsPath::from(path),
        None => match fs.home().await {
            Ok(home) => home,
            Err(_) => return StartFolder::Unchecked,
        },
    };
    let shown = configured.map_or_else(|| path.to_string(), str::to_string);
    match fs.stat(&path).await {
        Ok(metadata) if metadata.is_dir() => StartFolder::Opens(shown),
        Ok(_) => StartFolder::Unreadable { path: shown, message: "not a folder".to_string() },
        Err(error) if error.kind() == ErrorKind::NotFound => StartFolder::Missing(shown),
        Err(error) => StartFolder::Unreadable { path: shown, message: error.to_string() },
    }
}

#[cfg(test)]
mod tests;

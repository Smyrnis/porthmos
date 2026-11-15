use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

use porthmos_vfs::{FileSystem, ProtocolError, VfsPath};
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncReadExt;

use super::PROGRESS_INTERVAL;

const CHUNK: usize = 64 * 1024;
const CANCEL_POLL: std::time::Duration = std::time::Duration::from_millis(100);
const HEX_LENGTH: usize = 64;
const PREFIX: &str = "sha256:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest {
    pub hex: String,
    pub bytes: u64,
}

pub async fn sha256(
    fs: &dyn FileSystem, path: &VfsPath, cancel: &AtomicBool, progress: &mut (dyn FnMut(u64) + Send),
) -> Result<Option<Digest>, ProtocolError> {
    sha256_with_interval(fs, path, cancel, progress, PROGRESS_INTERVAL).await
}

pub(crate) async fn sha256_with_interval(
    fs: &dyn FileSystem, path: &VfsPath, cancel: &AtomicBool, progress: &mut (dyn FnMut(u64) + Send),
    interval: std::time::Duration,
) -> Result<Option<Digest>, ProtocolError> {
    let mut reader = fs.open_read(path, 0).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK];
    let mut bytes = 0u64;
    let mut last_report = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let Ok(read) = tokio::time::timeout(CANCEL_POLL, reader.read(&mut buffer)).await else {
            continue;
        };
        let read = read.map_err(ProtocolError::from)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes += read as u64;
        if last_report.elapsed() >= interval {
            progress(bytes);
            last_report = Instant::now();
        }
    }
    progress(bytes);
    Ok(Some(Digest { hex: format!("{:x}", hasher.finalize()), bytes }))
}

fn digest_token(expected: &str) -> Option<&str> {
    let line = expected.trim_start().trim_start_matches('\\');
    let after_equals = line.split_once(" = ").map(|(_, digest)| digest);
    after_equals.unwrap_or(line).split_whitespace().next()
}

pub fn matches_expected(expected: &str, actual_hex: &str) -> bool {
    let Some(token) = digest_token(expected) else {
        return false;
    };
    let digits = match token.get(..PREFIX.len()) {
        Some(head) if head.eq_ignore_ascii_case(PREFIX) => &token[PREFIX.len()..],
        _ => token,
    };
    digits.len() == HEX_LENGTH
        && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        && digits.eq_ignore_ascii_case(actual_hex)
}

#[cfg(test)]
mod tests;

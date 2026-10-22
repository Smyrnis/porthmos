use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use porthmos_vfs::{ErrorKind, FileSystem, Metadata, ProtocolError, Reader};
use tokio::{io::AsyncReadExt, time::Instant};

pub const PREVIEW_LIMIT: usize = 256 * 1024;
pub const STALL_TIMEOUT: Duration = Duration::from_secs(15);
pub const OPEN_TIMEOUT: Duration = Duration::from_secs(45);
const TICK: Duration = Duration::from_millis(100);
const CHUNK: usize = 16 * 1024;
const MODE_TYPE_MASK: u32 = 0o170000;
const SPECIAL_MODE_TYPES: [u32; 4] = [0o010000, 0o020000, 0o060000, 0o140000];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewContent {
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub size: Option<u64>,
    pub shown: usize,
    pub truncated: bool,
    pub content: PreviewContent,
}

pub fn classify(mut bytes: Vec<u8>, truncated: bool) -> PreviewContent {
    if bytes.contains(&0) {
        return PreviewContent::Binary(bytes);
    }
    let valid = match std::str::from_utf8(&bytes) {
        Ok(_) => bytes.len(),
        Err(error) if truncated && error.error_len().is_none() => error.valid_up_to(),
        Err(_) => return PreviewContent::Binary(bytes),
    };
    bytes.truncate(valid);
    match String::from_utf8(bytes) {
        Ok(mut text) => {
            if text.starts_with('\u{feff}') {
                text.drain(..'\u{feff}'.len_utf8());
            }
            PreviewContent::Text(text)
        }
        Err(error) => PreviewContent::Binary(error.into_bytes()),
    }
}

pub(crate) fn is_special(mode: u32) -> bool {
    SPECIAL_MODE_TYPES.contains(&(mode & MODE_TYPE_MASK))
}

pub(crate) fn known_size(reported: u64, shown: usize) -> Option<u64> {
    (reported >= shown as u64).then_some(reported)
}

fn too_slow() -> ProtocolError {
    ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("the server took too long to answer"))
}

fn cancelled() -> ProtocolError {
    ProtocolError::new(ErrorKind::Cancelled, anyhow::anyhow!("cancelled"))
}

fn check_readable(path: &Path, metadata: &Metadata) -> Result<(), ProtocolError> {
    if metadata.is_dir() {
        return Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("{} is a folder", path.display())));
    }
    if metadata.permissions.is_some_and(is_special) {
        return Err(ProtocolError::new(ErrorKind::Other, anyhow::anyhow!("{} is not a regular file", path.display())));
    }
    Ok(())
}

async fn read_some(reader: &mut Reader, buffer: &mut [u8], cancel: &AtomicBool) -> Result<usize, ProtocolError> {
    let started = Instant::now();
    let read = reader.read(buffer);
    tokio::pin!(read);
    loop {
        tokio::select! {
            outcome = &mut read => return outcome.map_err(ProtocolError::from),
            () = tokio::time::sleep(TICK) => {
                if cancel.load(Ordering::Relaxed) {
                    return Err(cancelled());
                }
                if started.elapsed() >= STALL_TIMEOUT {
                    return Err(too_slow());
                }
            }
        }
    }
}

pub async fn read_preview(
    fs: &dyn FileSystem, path: &Path, limit: usize, cancel: &AtomicBool,
) -> Result<Preview, ProtocolError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(cancelled());
    }
    let opened = tokio::time::timeout(OPEN_TIMEOUT, async {
        let metadata = fs.stat(path).await?;
        check_readable(path, &metadata)?;
        let reader = fs.open_read(path, 0).await?;
        Ok::<_, ProtocolError>((metadata, reader))
    })
    .await;
    let (metadata, mut reader) = opened.map_err(|_| too_slow())??;

    let mut bytes = Vec::new();
    let mut chunk = vec![0u8; CHUNK];
    while bytes.len() <= limit {
        let wanted = (limit + 1 - bytes.len()).min(CHUNK);
        let read = read_some(&mut reader, &mut chunk[..wanted], cancel).await?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    let shown = bytes.len();
    Ok(Preview { size: known_size(metadata.size, shown), shown, truncated, content: classify(bytes, truncated) })
}

#[cfg(test)]
mod tests;

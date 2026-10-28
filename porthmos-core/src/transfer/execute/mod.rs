use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use porthmos_vfs::{FileSystem, PART_SUFFIX, ProtocolError, VfsPath};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::Instant,
};

use super::limit::{MAX_CHUNK, SpeedLimit};

const PROGRESS_STEP_BYTES: u64 = 64 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
const CLOCK_SKEW_TOLERANCE_SECS: u64 = 120;

#[derive(Debug, PartialEq, Eq)]
pub enum TransferOutcome {
    Completed,
    Cancelled,
}

pub struct Executed {
    pub outcome: TransferOutcome,
    pub times_kept: bool,
}

pub async fn execute(
    source: &dyn FileSystem, source_path: &VfsPath, destination: &dyn FileSystem, destination_path: &VfsPath,
    cancel: &AtomicBool, resume: bool, on_progress: impl FnMut(u64) + Send,
) -> Result<TransferOutcome, ProtocolError> {
    execute_preserving(
        source,
        source_path,
        destination,
        destination_path,
        cancel,
        resume,
        None,
        &SpeedLimit::new(),
        on_progress,
    )
    .await
    .map(|executed| executed.outcome)
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_preserving(
    source: &dyn FileSystem, source_path: &VfsPath, destination: &dyn FileSystem, destination_path: &VfsPath,
    cancel: &AtomicBool, resume: bool, keep_modified: Option<u64>, limit: &SpeedLimit,
    on_progress: impl FnMut(u64) + Send,
) -> Result<Executed, ProtocolError> {
    let source_metadata = source.stat(source_path).await?;
    let part = part_path(destination_path);
    let offset = if resume {
        let backoff = source.resume_backoff().max(destination.resume_backoff());
        match destination.stat(&part).await {
            Ok(part_metadata) => resume_offset(
                part_metadata.size,
                source_metadata.size,
                part_metadata.modified,
                source_metadata.modified,
                backoff,
            ),
            Err(_) => 0,
        }
    } else {
        0
    };
    let mut writer = destination.open_write_sized(&part, offset, source_metadata.size).await?;
    let mut reader = match source.open_read(source_path, writer.offset).await {
        Ok(reader) => reader,
        Err(err) => {
            let _ = writer.stream.shutdown().await;
            discard_empty_part(destination, &part).await;
            return Err(err);
        }
    };
    let result = copy_with_progress(&mut reader, &mut writer.stream, writer.offset, cancel, limit, on_progress).await;
    let shutdown = writer.stream.shutdown().await;
    let result = match (result, shutdown) {
        (Ok(TransferOutcome::Completed), Err(err)) => Err(ProtocolError::from(err)),
        (result, _) => result,
    };
    let mut times_kept = true;
    if let Ok(TransferOutcome::Completed) = result {
        destination.rename(&part, destination_path).await?;
        if let Some(seconds) = keep_modified {
            times_kept = destination.set_modified(destination_path, seconds).await.is_ok();
        }
    }
    result.map(|outcome| Executed { outcome, times_kept })
}

async fn discard_empty_part(destination: &dyn FileSystem, part: &VfsPath) {
    if destination.stat(part).await.is_ok_and(|metadata| metadata.size == 0) {
        let _ = destination.remove_file(part).await;
    }
}

pub(crate) fn resume_offset(
    part_len: u64, source_len: u64, part_modified: Option<u64>, source_modified: Option<u64>, backoff: u64,
) -> u64 {
    let (Some(part_modified), Some(source_modified)) = (part_modified, source_modified) else {
        return 0;
    };
    if part_len > source_len || source_modified > part_modified + CLOCK_SKEW_TOLERANCE_SECS {
        return 0;
    }
    part_len.saturating_sub(backoff)
}

async fn copy_with_progress<R, W>(
    source: &mut R, destination: &mut W, start: u64, cancel: &AtomicBool, limit: &SpeedLimit,
    mut on_progress: impl FnMut(u64),
) -> Result<TransferOutcome, ProtocolError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; MAX_CHUNK];
    let mut transferred: u64 = start;
    let mut since_last_report: u64 = 0;
    on_progress(start);
    let mut reported_at = Instant::now();

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(TransferOutcome::Cancelled);
        }

        let chunk = limit.chunk_cap().min(buf.len());
        let bytes_read = source.read(&mut buf[..chunk]).await?;
        if bytes_read == 0 {
            break;
        }
        if !limit.throttle(bytes_read as u64, cancel).await {
            return Ok(TransferOutcome::Cancelled);
        }

        destination.write_all(&buf[..bytes_read]).await?;
        transferred += bytes_read as u64;
        since_last_report += bytes_read as u64;

        if since_last_report >= PROGRESS_STEP_BYTES || reported_at.elapsed() >= PROGRESS_INTERVAL {
            on_progress(transferred);
            since_last_report = 0;
            reported_at = Instant::now();
        }
    }

    destination.flush().await?;
    on_progress(transferred);
    Ok(TransferOutcome::Completed)
}

pub fn part_path(path: &VfsPath) -> VfsPath {
    path.with_suffix(PART_SUFFIX)
}

#[cfg(test)]
mod tests;

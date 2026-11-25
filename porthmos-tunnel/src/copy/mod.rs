use std::{
    io,
    sync::atomic::{AtomicU64, Ordering},
};

use porthmos_vfs::ForwardStream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::stats::Stats;

const CHUNK: usize = 16 * 1024;

async fn counted<R, W>(mut reader: R, mut writer: W, counter: &AtomicU64) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            let _ = writer.shutdown().await;
            return Ok(());
        }
        writer.write_all(&buffer[..count]).await?;
        counter.fetch_add(count as u64, Ordering::Relaxed);
    }
}

pub(crate) async fn pipe<C>(client: C, remote: ForwardStream, stats: &Stats) -> io::Result<()>
where
    C: AsyncRead + AsyncWrite + Unpin,
{
    let (client_read, client_write) = tokio::io::split(client);
    let (remote_read, remote_write) = tokio::io::split(remote);
    tokio::try_join!(
        counted(client_read, remote_write, &stats.bytes_up),
        counted(remote_read, client_write, &stats.bytes_down),
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests;

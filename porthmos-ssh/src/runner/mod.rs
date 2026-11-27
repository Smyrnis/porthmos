use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use anyhow::anyhow;
use porthmos_vfs::{CommandOutput, CommandRunner, CommandStream, ErrorKind, ProtocolError, StreamEnd, async_trait};
use tokio::{
    io::{AsyncRead, AsyncReadExt, DuplexStream, ReadBuf},
    sync::oneshot,
    task::JoinHandle,
};

use crate::{Session, open_exec};

const MAX_OUTPUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_STREAM_STDERR_BYTES: usize = 4 * 1024;

pub struct SshRunner {
    session: Arc<Session>,
}

impl SshRunner {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

fn failed(reason: impl std::fmt::Display) -> ProtocolError {
    ProtocolError::new(ErrorKind::Other, anyhow!("{reason}"))
}

struct StreamReader {
    output: DuplexStream,
    task: JoinHandle<()>,
    reached_end: bool,
}

impl AsyncRead for StreamReader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let had_room = buf.remaining() > 0;
        let polled = Pin::new(&mut self.output).poll_read(cx, buf);
        if had_room && matches!(polled, Poll::Ready(Ok(()))) && buf.filled().len() == before {
            self.reached_end = true;
        }
        polled
    }
}

impl Drop for StreamReader {
    fn drop(&mut self) {
        if !self.reached_end {
            self.task.abort();
        }
    }
}

#[async_trait]
impl CommandRunner for SshRunner {
    async fn run_with_input(&self, command: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError> {
        let mut channel = open_exec(&self.session, command).await?;
        if !input.is_empty() {
            channel.input.send(input).await.map_err(failed)?;
        }
        channel.input.close().await.map_err(failed)?;
        let mut stdout = Vec::new();
        (&mut channel.output).take(MAX_OUTPUT_BYTES + 1).read_to_end(&mut stdout).await.map_err(failed)?;
        if stdout.len() as u64 > MAX_OUTPUT_BYTES {
            return Err(failed("the command printed too much output"));
        }
        let finished = channel.finish().await;
        let Some(status) = finished.status else {
            return Err(failed("the server did not run the command"));
        };
        Ok(CommandOutput { stdout, stderr: finished.stderr, status: Some(status) })
    }
    async fn open_stream(&self, command: &str, input: &[u8]) -> Result<CommandStream, ProtocolError> {
        let mut channel = open_exec(&self.session, command).await?;
        if !input.is_empty() {
            channel.input.send(input).await.map_err(failed)?;
        }
        channel.input.close().await.map_err(failed)?;
        channel.input.terminate_when_abandoned();
        let output = std::mem::replace(&mut channel.output, tokio::io::duplex(1).1);
        let (ended_tx, ended) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut finished = channel.finish().await;
            finished.stderr.truncate(MAX_STREAM_STDERR_BYTES);
            let _ = ended_tx.send(StreamEnd { status: finished.status, stderr: finished.stderr });
        });
        Ok(CommandStream { output: Box::new(StreamReader { output, task, reached_end: false }), ended })
    }
}

#[cfg(test)]
mod tests;

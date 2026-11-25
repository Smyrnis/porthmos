use std::io;

use anyhow::anyhow;
use porthmos_vfs::{ErrorKind, ProtocolError};
use russh::{ChannelMsg, client::Msg};
use tokio::{
    io::{AsyncWriteExt, DuplexStream},
    sync::watch,
    task::JoinHandle,
};

use crate::{Session, exec::ExecInput};

const OUTPUT_BUFFER: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellSize {
    pub columns: u16,
    pub rows: u16,
}

pub struct ShellInput {
    channel: ExecInput,
}

impl ShellInput {
    pub async fn send(&self, data: &[u8]) -> io::Result<()> {
        self.channel.send(data).await
    }

    pub async fn resize(&self, size: ShellSize) -> io::Result<()> {
        self.channel.window_change(size.columns, size.rows).await
    }

    pub async fn close(&self) -> io::Result<()> {
        if self.channel.has_ended() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "the shell's channel is closed"));
        }
        self.channel.close().await
    }
}

pub struct ShellChannel {
    pub input: ShellInput,
    pub output: DuplexStream,
    done: Option<JoinHandle<Option<u32>>>,
    finished: bool,
}

impl ShellChannel {
    pub async fn finish(mut self) -> Option<u32> {
        drop(std::mem::replace(&mut self.output, tokio::io::duplex(1).1));
        let status = match self.done.take() {
            Some(done) if done.is_finished() => done.await.unwrap_or_default(),
            Some(done) => {
                self.input.channel.abandon();
                done.abort();
                None
            }
            None => None,
        };
        self.finished = true;
        status
    }
}

impl Drop for ShellChannel {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.input.channel.abandon();
        if let Some(done) = self.done.take() {
            done.abort();
        }
    }
}

fn shell_error(err: impl Into<anyhow::Error>) -> ProtocolError {
    ProtocolError::new(ErrorKind::Other, err.into())
}

async fn accepted(channel: &mut russh::Channel<Msg>, request: &str) -> Result<(), ProtocolError> {
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Success) => return Ok(()),
            Some(ChannelMsg::Failure) | Some(ChannelMsg::Close) | None => {
                return Err(shell_error(anyhow!("the server refused the {request} request")));
            }
            Some(_) => {}
        }
    }
}

pub async fn open_shell(
    session: &Session, term: &str, size: ShellSize, env: &[(&str, &str)],
) -> Result<ShellChannel, ProtocolError> {
    let mut channel = session.handle().channel_open_session().await.map_err(shell_error)?;
    channel
        .request_pty(true, term, u32::from(size.columns), u32::from(size.rows), 0, 0, &[])
        .await
        .map_err(shell_error)?;
    accepted(&mut channel, "pty").await?;
    for (name, value) in env {
        let _ = channel.set_env(false, *name, *value).await;
    }
    channel.request_shell(true).await.map_err(shell_error)?;
    accepted(&mut channel, "shell").await?;
    let (mut read, write) = channel.split();
    let (mut sink, output) = tokio::io::duplex(OUTPUT_BUFFER);
    let (ended, closed) = watch::channel(false);
    let done = tokio::spawn(async move {
        let mut status = None;
        let mut forwarding = true;
        while let Some(message) = read.wait().await {
            match message {
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } if forwarding => {
                    forwarding = sink.write_all(&data).await.is_ok();
                }
                ChannelMsg::ExitStatus { exit_status } => status = Some(exit_status),
                ChannelMsg::Failure | ChannelMsg::Close => break,
                _ => {}
            }
        }
        let _ = ended.send(true);
        status
    });
    Ok(ShellChannel {
        input: ShellInput { channel: ExecInput::new(write, closed) },
        output,
        done: Some(done),
        finished: false,
    })
}

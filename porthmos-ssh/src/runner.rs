use std::sync::Arc;

use anyhow::anyhow;
use porthmos_vfs::{CommandOutput, CommandRunner, ErrorKind, ProtocolError, async_trait};
use tokio::io::AsyncReadExt;

use crate::{Session, open_exec};

const MAX_OUTPUT_BYTES: u64 = 4 * 1024 * 1024;

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
}

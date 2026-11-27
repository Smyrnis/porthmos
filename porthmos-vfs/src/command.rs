use anyhow::anyhow;
use async_trait::async_trait;
use tokio::{io::AsyncRead, sync::oneshot};

use crate::{ErrorKind, ProtocolError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamEnd {
    pub status: Option<u32>,
    pub stderr: Vec<u8>,
}

pub struct CommandStream {
    pub output: Box<dyn AsyncRead + Send + Unpin>,
    pub ended: oneshot::Receiver<StreamEnd>,
}

#[async_trait]
pub trait CommandRunner: Send + Sync {
    async fn run_with_input(&self, command: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError>;

    async fn run(&self, command: &str) -> Result<CommandOutput, ProtocolError> {
        self.run_with_input(command, &[]).await
    }

    async fn open_stream(&self, _command: &str, _input: &[u8]) -> Result<CommandStream, ProtocolError> {
        Err(ProtocolError::new(ErrorKind::Unsupported, anyhow!("this connection can't follow a command")))
    }
}

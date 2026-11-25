use async_trait::async_trait;

use crate::ProtocolError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: Option<u32>,
}

#[async_trait]
pub trait CommandRunner: Send + Sync {
    async fn run_with_input(&self, command: &str, input: &[u8]) -> Result<CommandOutput, ProtocolError>;

    async fn run(&self, command: &str) -> Result<CommandOutput, ProtocolError> {
        self.run_with_input(command, &[]).await
    }
}

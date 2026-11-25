use std::sync::Arc;

use anyhow::anyhow;
use porthmos_vfs::{ErrorKind, ForwardStream, PortForwarder, ProtocolError, async_trait};
use russh::ChannelOpenFailure;

use crate::Session;

pub struct SshForwarder {
    session: Arc<Session>,
}

impl SshForwarder {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl PortForwarder for SshForwarder {
    async fn connect(&self, host: &str, port: u16) -> Result<ForwardStream, ProtocolError> {
        match self.session.handle().channel_open_direct_tcpip(host, u32::from(port), "127.0.0.1", 0).await {
            Ok(channel) => Ok(Box::new(channel.into_stream())),
            Err(russh::Error::ChannelOpenFailure(ChannelOpenFailure::AdministrativelyProhibited)) => {
                Err(ProtocolError::new(ErrorKind::Unsupported, anyhow!("the server does not allow port forwarding")))
            }
            Err(err) => Err(ProtocolError::new(ErrorKind::Connect, anyhow!("could not reach {host}:{port}: {err}"))),
        }
    }
}

use std::net::IpAddr;

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc,
};

use crate::{ErrorKind, ProtocolError};

pub trait AsyncDuplex: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> AsyncDuplex for T {}

pub type ForwardStream = Box<dyn AsyncDuplex>;

pub struct IncomingConnection {
    pub stream: ForwardStream,
    pub origin: String,
}

#[async_trait]
pub trait ListenerCloser: Send + Sync {
    async fn close(&self);
}

pub struct RemoteListener {
    pub port: u16,
    pub incoming: mpsc::Receiver<IncomingConnection>,
    closer: Box<dyn ListenerCloser>,
}

impl RemoteListener {
    pub fn new(port: u16, incoming: mpsc::Receiver<IncomingConnection>, closer: Box<dyn ListenerCloser>) -> Self {
        Self { port, incoming, closer }
    }

    pub async fn close(self) {
        self.closer.close().await;
    }
}

#[async_trait]
pub trait PortForwarder: Send + Sync {
    async fn connect(&self, host: &str, port: u16) -> Result<ForwardStream, ProtocolError>;

    async fn listen(&self, _bind: IpAddr, _port: u16) -> Result<RemoteListener, ProtocolError> {
        Err(ProtocolError::new(ErrorKind::Unsupported, anyhow!("this connection cannot listen for remote forwards")))
    }
}

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::ProtocolError;

pub trait AsyncDuplex: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> AsyncDuplex for T {}

pub type ForwardStream = Box<dyn AsyncDuplex>;

#[async_trait]
pub trait PortForwarder: Send + Sync {
    async fn connect(&self, host: &str, port: u16) -> Result<ForwardStream, ProtocolError>;
}

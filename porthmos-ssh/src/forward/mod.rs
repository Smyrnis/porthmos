use std::{net::IpAddr, sync::Arc};

use anyhow::anyhow;
use porthmos_vfs::{
    ErrorKind, ForwardStream, ListenerCloser, PortForwarder, ProtocolError, RemoteListener, async_trait,
};
use russh::ChannelOpenFailure;
use tokio::sync::mpsc;

use crate::{Session, forwards::ForwardRegistry};

const INCOMING_BACKLOG: usize = 16;

pub struct SshForwarder {
    session: Arc<Session>,
}

impl SshForwarder {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

struct CancelForward {
    session: Arc<Session>,
    registry: ForwardRegistry,
    address: String,
    port: u32,
}

#[async_trait]
impl ListenerCloser for CancelForward {
    async fn close(&self) {
        self.registry.unregister(&self.address, self.port);
        let _ = self.session.handle().cancel_tcpip_forward(self.address.clone(), self.port).await;
    }
}

fn granted_port(requested: u16, answered: u32) -> Result<u16, ProtocolError> {
    if requested != 0 {
        return Ok(requested);
    }
    match u16::try_from(answered) {
        Ok(0) | Err(_) => Err(ProtocolError::new(
            ErrorKind::Connect,
            anyhow!("the server did not say which port it chose (it answered {answered})"),
        )),
        Ok(port) => Ok(port),
    }
}

fn listen_error(error: russh::Error) -> ProtocolError {
    match error {
        russh::Error::RequestDenied => ProtocolError::new(
            ErrorKind::Unsupported,
            anyhow!("the server did not allow the remote forward (it may be disabled or the port may be in use)"),
        ),
        other => ProtocolError::new(ErrorKind::Connect, anyhow!("could not ask the server to listen: {other}")),
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

    async fn listen(&self, bind: IpAddr, port: u16) -> Result<RemoteListener, ProtocolError> {
        let address = bind.to_string();
        let registry = self.session.forwards();
        let (sender, incoming) = mpsc::channel(INCOMING_BACKLOG);
        let requested = u32::from(port);
        if port != 0 {
            if registry.route(&address, requested).is_some() {
                return Err(ProtocolError::new(
                    ErrorKind::Unsupported,
                    anyhow!("this connection is already listening on {address}:{port}"),
                ));
            }
            registry.register(&address, requested, sender.clone());
        }
        let answered = match self.session.handle().tcpip_forward(address.clone(), requested).await {
            Ok(granted) => granted,
            Err(error) => {
                if port != 0 {
                    registry.unregister(&address, requested);
                }
                return Err(listen_error(error));
            }
        };
        let reported = match granted_port(port, answered) {
            Ok(reported) => reported,
            Err(error) => {
                let _ = self.session.handle().cancel_tcpip_forward(address, answered).await;
                return Err(error);
            }
        };
        let granted = u32::from(reported);
        registry.register(&address, granted, sender);
        let closer = CancelForward { session: self.session.clone(), registry, address, port: granted };
        Ok(RemoteListener::new(reported, incoming, Box::new(closer)))
    }
}

#[cfg(test)]
mod tests;

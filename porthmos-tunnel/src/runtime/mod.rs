use std::{
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use porthmos_vfs::{ErrorKind, ForwardStream, PortForwarder};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};

use crate::{
    copy::pipe,
    socks::{self, Reply, Request},
    spec::{Kind, SpecError, TunnelSpec},
    stats::{Stats, StatsSnapshot},
};

pub const MAX_CONNECTIONS: usize = 256;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const ACCEPT_RETRY: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub enum StartError {
    Invalid(SpecError),
    Bind { address: String, reason: String },
}

pub struct RunningTunnel {
    local: SocketAddr,
    stats: Stats,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl RunningTunnel {
    pub async fn start(spec: &TunnelSpec, forwarder: Arc<dyn PortForwarder>) -> Result<Self, StartError> {
        spec.validate().map_err(StartError::Invalid)?;
        let listen = spec.kind.listen();
        let listener = TcpListener::bind(SocketAddr::new(listen.address, listen.port))
            .await
            .map_err(|err| StartError::Bind { address: listen.to_string(), reason: err.to_string() })?;
        Ok(serve(listener, spec.kind.clone(), forwarder))
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    pub async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = (&mut self.task).await;
    }
}

pub(crate) fn serve(listener: TcpListener, kind: Kind, forwarder: Arc<dyn PortForwarder>) -> RunningTunnel {
    let local = listener.local_addr().unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0)));
    let stats = Stats::default();
    let (stop, stopped) = oneshot::channel();
    let task = tokio::spawn(accept_loop(listener, kind, forwarder, stats.clone(), stopped));
    RunningTunnel { local, stats, stop: Some(stop), task }
}

async fn accept_loop(
    listener: TcpListener, kind: Kind, forwarder: Arc<dyn PortForwarder>, stats: Stats,
    mut stopped: oneshot::Receiver<()>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = &mut stopped => break,
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept() => match accepted {
                Ok((client, _)) => {
                    if stats.active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                        stats.refused.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    let guard = stats.open();
                    let (kind, forwarder, stats) = (kind.clone(), forwarder.clone(), stats.clone());
                    connections.spawn(async move {
                        let _guard = guard;
                        handle(client, &kind, forwarder, &stats).await;
                    });
                }
                Err(_) => tokio::time::sleep(ACCEPT_RETRY).await,
            },
        }
    }
    connections.shutdown().await;
}

async fn open(forwarder: &dyn PortForwarder, host: &str, port: u16) -> Result<ForwardStream, Reply> {
    match tokio::time::timeout(CONNECT_TIMEOUT, forwarder.connect(host, port)).await {
        Ok(Ok(remote)) => Ok(remote),
        Ok(Err(err)) => Err(match err.kind() {
            ErrorKind::Connect => Reply::ConnectionRefused,
            ErrorKind::Unsupported => Reply::ConnectionNotAllowed,
            _ => Reply::GeneralFailure,
        }),
        Err(_) => Err(Reply::GeneralFailure),
    }
}

async fn handle(mut client: TcpStream, kind: &Kind, forwarder: Arc<dyn PortForwarder>, stats: &Stats) {
    let opened = match kind {
        Kind::Local { target, .. } => open(forwarder.as_ref(), &target.host, target.port).await,
        Kind::Dynamic { .. } => {
            let Ok(Request::Connect(endpoint)) = socks::handshake(&mut client).await else {
                stats.errors.fetch_add(1, Ordering::Relaxed);
                return;
            };
            let opened = open(forwarder.as_ref(), &endpoint.host, endpoint.port).await;
            socks::reply(&mut client, opened.as_ref().err().copied().unwrap_or(Reply::Succeeded)).await;
            opened
        }
    };
    match opened {
        Ok(remote) => {
            let _ = pipe(client, remote, stats).await;
        }
        Err(_) => {
            stats.errors.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests;

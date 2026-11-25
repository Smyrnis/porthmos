use std::{
    net::IpAddr,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{
    ErrorKind, ForwardStream, IncomingConnection, ListenerCloser, PortForwarder, ProtocolError, RemoteListener,
};

const DEFAULT_ALLOCATED_PORT: u16 = 40000;
const INCOMING_CAPACITY: usize = 512;

struct State {
    sender: Option<mpsc::Sender<IncomingConnection>>,
    closed: bool,
    requests: Vec<(IpAddr, u16)>,
    refusal: Option<String>,
    allocated: u16,
}

type Shared = Arc<Mutex<State>>;

pub struct FakeRemote {
    state: Shared,
}

struct Closer(Shared);

fn locked(state: &Shared) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[async_trait]
impl ListenerCloser for Closer {
    async fn close(&self) {
        let mut state = locked(&self.0);
        state.closed = true;
        state.sender = None;
    }
}

impl FakeRemote {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Arc::new(Mutex::new(State {
                sender: None,
                closed: false,
                requests: Vec::new(),
                refusal: None,
                allocated: DEFAULT_ALLOCATED_PORT,
            })),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        locked(&self.state)
    }

    pub fn push(&self, stream: ForwardStream, origin: &str) {
        let sender = self.state().sender.clone();
        if let Some(sender) = sender {
            let _ = sender.try_send(IncomingConnection { stream, origin: origin.to_string() });
        }
    }

    pub fn end_incoming(&self) {
        self.state().sender = None;
    }

    pub fn closed(&self) -> bool {
        self.state().closed
    }

    pub fn requests(&self) -> Vec<(IpAddr, u16)> {
        self.state().requests.clone()
    }

    pub fn refuse_with(&self, message: &str) {
        self.state().refusal = Some(message.to_string());
    }

    pub fn allocate(&self, port: u16) {
        self.state().allocated = port;
    }
}

#[async_trait]
impl PortForwarder for FakeRemote {
    async fn connect(&self, _host: &str, _port: u16) -> Result<ForwardStream, ProtocolError> {
        Err(ProtocolError::new(ErrorKind::Unsupported, anyhow!("the fake remote only listens")))
    }

    async fn listen(&self, bind: IpAddr, port: u16) -> Result<RemoteListener, ProtocolError> {
        let (sender, incoming) = mpsc::channel(INCOMING_CAPACITY);
        let mut state = self.state();
        state.requests.push((bind, port));
        if let Some(message) = &state.refusal {
            return Err(ProtocolError::new(ErrorKind::Connect, anyhow!("{message}")));
        }
        let granted = if port == 0 { state.allocated } else { port };
        state.sender = Some(sender);
        state.closed = false;
        Ok(RemoteListener::new(granted, incoming, Box::new(Closer(self.state.clone()))))
    }
}

use porthmos_core::i18n;
use porthmos_core::{
    NetInterface, PortsSnapshot, PortsState as PortsEvent, SessionId, Socket, SocketProtocol, SocketState,
};

const MISSING: &str = "\u{2014}";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PortsPane {
    #[default]
    Listening,
    Connections,
    Interfaces,
}

#[derive(Debug, Default)]
pub struct PortsState {
    pub session: Option<SessionId>,
    pub started: Option<SessionId>,
    pub state: Option<PortsEvent>,
    pub pane: PortsPane,
    pub filter: String,
    kept: Option<Box<PortsSnapshot>>,
}

impl PortsState {
    pub fn begin(&mut self, session: SessionId) {
        self.session = Some(session);
        self.state = None;
        self.kept = None;
    }

    pub fn clear(&mut self) {
        self.session = None;
        self.state = None;
        self.kept = None;
    }

    pub fn apply(&mut self, session: SessionId, state: PortsEvent) {
        if self.session != Some(session) {
            return;
        }
        if let Some(PortsEvent::Snapshot(snapshot)) = self.state.take() {
            self.kept = Some(snapshot);
        }
        if !matches!(state, PortsEvent::Failing(_)) {
            self.kept = None;
        }
        self.state = Some(state);
    }

    pub fn snapshot(&self) -> Option<&PortsSnapshot> {
        match &self.state {
            Some(PortsEvent::Snapshot(snapshot)) => Some(snapshot),
            Some(PortsEvent::Failing(_)) => self.kept.as_deref(),
            _ => None,
        }
    }

    pub fn failing(&self) -> bool {
        matches!(self.state, Some(PortsEvent::Failing(_)))
    }
}

pub fn protocol_text(protocol: SocketProtocol) -> &'static str {
    match protocol {
        SocketProtocol::Tcp => "TCP",
        SocketProtocol::Udp => "UDP",
    }
}

pub fn socket_state_text(state: SocketState) -> &'static str {
    match state {
        SocketState::Listen => "LISTEN",
        SocketState::Established => "ESTABLISHED",
        SocketState::SynSent => "SYN_SENT",
        SocketState::SynRecv => "SYN_RECV",
        SocketState::FinWait1 => "FIN_WAIT1",
        SocketState::FinWait2 => "FIN_WAIT2",
        SocketState::TimeWait => "TIME_WAIT",
        SocketState::Close => "CLOSE",
        SocketState::CloseWait => "CLOSE_WAIT",
        SocketState::LastAck => "LAST_ACK",
        SocketState::Closing => "CLOSING",
        SocketState::Other => MISSING,
    }
}

pub fn owner_text(socket: &Socket) -> String {
    match &socket.owner {
        Some((pid, name)) => format!("{name} ({pid})"),
        None => MISSING.to_string(),
    }
}

pub fn owner_hint(uid: u32, socket: &Socket) -> Option<String> {
    (socket.owner.is_none() && uid != 0).then(|| i18n::text("gui.ports.run_as_root"))
}

pub fn interface_state_text(up: Option<bool>) -> String {
    match up {
        Some(true) => i18n::text("gui.ports.up"),
        Some(false) => i18n::text("gui.ports.down"),
        None => MISSING.to_string(),
    }
}

pub fn addresses_text(interface: &NetInterface) -> String {
    if interface.addresses.is_empty() { MISSING.to_string() } else { interface.addresses.join(", ") }
}

pub fn mac_text(interface: &NetInterface) -> String {
    interface.mac.clone().unwrap_or_else(|| MISSING.to_string())
}

pub fn mtu_text(interface: &NetInterface) -> String {
    interface.mtu.map_or_else(|| MISSING.to_string(), |mtu| mtu.to_string())
}

#[cfg(test)]
pub(crate) mod tests;

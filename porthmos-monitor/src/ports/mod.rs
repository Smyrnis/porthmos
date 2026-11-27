use crate::{
    UNKNOWN_UID,
    parse::{
        sections,
        split::{self, Name},
    },
    probe::Token,
};

mod interfaces;
mod script;
mod sockets;

pub use script::ports_command;

pub const MAX_SOCKETS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SocketProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SocketState {
    Listen,
    Established,
    SynSent,
    SynRecv,
    FinWait1,
    FinWait2,
    TimeWait,
    Close,
    CloseWait,
    LastAck,
    Closing,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socket {
    pub protocol: SocketProtocol,
    pub local: String,
    pub remote: String,
    pub state: SocketState,
    pub owner: Option<(u32, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetInterface {
    pub name: String,
    pub up: Option<bool>,
    pub addresses: Vec<String>,
    pub mac: Option<String>,
    pub mtu: Option<u32>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortsSection {
    Sockets,
    Interfaces,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortsSnapshot {
    pub listening: Vec<Socket>,
    pub connections: Vec<Socket>,
    pub interfaces: Vec<NetInterface>,
    pub uid: u32,
    pub problems: Vec<PortsSection>,
}

#[derive(Debug)]
pub enum ParsedPorts {
    Linux(Box<PortsSnapshot>),
    Unsupported { os: String },
    Garbled,
}

pub fn parse_ports(output: &[u8], token: &Token) -> ParsedPorts {
    let split = split::split(output, token);
    let Some(os) = split.first_line(Name::Os).and_then(|line| sections::text(line, 64)) else {
        return ParsedPorts::Garbled;
    };
    if os != "Linux" {
        return ParsedPorts::Unsupported { os };
    }
    let mut problems = Vec::new();
    let sockets = sockets::sockets(&split);
    if sockets.is_none() {
        problems.push(PortsSection::Sockets);
    }
    let (listening, connections) = sockets.unwrap_or_default();
    let interfaces = interfaces::interfaces(&split);
    if interfaces.is_none() {
        problems.push(PortsSection::Interfaces);
    }
    let uid = split
        .first_line(Name::Uid)
        .and_then(|line| String::from_utf8_lossy(line).trim().parse::<u32>().ok())
        .unwrap_or(UNKNOWN_UID);
    ParsedPorts::Linux(Box::new(PortsSnapshot {
        listening,
        connections,
        interfaces: interfaces.unwrap_or_default(),
        uid,
        problems,
    }))
}

fn port(local: &str) -> u16 {
    local.rsplit_once(':').and_then(|(_, port)| port.parse().ok()).unwrap_or(0)
}

pub(crate) fn contains_ignoring_case(text: &str, needle: &str) -> bool {
    if needle.is_ascii() {
        let wanted = needle.as_bytes();
        return text.as_bytes().windows(wanted.len()).any(|window| window.eq_ignore_ascii_case(wanted));
    }
    text.to_lowercase().contains(needle)
}

fn matches(socket: &Socket, needle: &str) -> bool {
    needle.is_empty()
        || [
            Some(socket.local.as_str()),
            Some(socket.remote.as_str()),
            socket.owner.as_ref().map(|(_, name)| name.as_str()),
        ]
        .into_iter()
        .flatten()
        .any(|text| contains_ignoring_case(text, needle))
}

pub fn arranged_sockets<'a>(all: &'a [Socket], filter: &str) -> Vec<&'a Socket> {
    let needle = filter.trim().to_lowercase();
    let mut shown: Vec<&Socket> = all.iter().filter(|socket| matches(socket, &needle)).collect();
    shown.sort_by_cached_key(|socket| {
        let listening = socket.state == SocketState::Listen;
        (
            !listening,
            if listening { port(&socket.local) } else { 0 },
            socket.state,
            socket.remote.clone(),
            socket.protocol,
            socket.local.clone(),
        )
    });
    shown
}

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, hash_map::Entry},
    net::{Ipv4Addr, Ipv6Addr},
};

use super::{MAX_SOCKETS, Socket, SocketProtocol, SocketState};
use crate::{
    MAX_LINE_BYTES,
    parse::{
        sections,
        split::{Name, Split},
    },
};

const NAME_LIMIT: usize = 32;
const COLUMNS: usize = 10;
const LOCAL: usize = 1;
const REMOTE: usize = 2;
const STATE: usize = 3;
const INODE: usize = 9;
const NO_REMOTE: [&str; 2] = ["0.0.0.0:0", "[::]:0"];

struct Owners {
    pids: HashMap<u64, u32>,
    names: HashMap<u32, Option<String>>,
}

impl Owners {
    fn of(&self, inode: u64) -> Option<(u32, String)> {
        let pid = *self.pids.get(&inode)?;
        Some((pid, self.names.get(&pid)?.clone()?))
    }
}

fn socket_inode(line: &str) -> Option<u64> {
    let (_, target) = line.split_once(" -> ")?;
    target.trim_matches('\'').strip_prefix("socket:[")?.strip_suffix(']')?.parse().ok()
}

fn owners(split: &Split) -> Owners {
    let mut pids = HashMap::new();
    let mut current = None;
    for line in split.lines(Name::FdLinks).unwrap_or_default() {
        let Ok(text) = std::str::from_utf8(line) else { continue };
        if let Some(header) = text.strip_prefix("/proc/") {
            current = header.strip_suffix("/fd:").and_then(|pid| pid.parse::<u32>().ok());
        } else if let (Some(pid), Some(inode)) = (current, socket_inode(text)) {
            pids.entry(inode).and_modify(|lowest: &mut u32| *lowest = (*lowest).min(pid)).or_insert(pid);
        }
    }
    let mut names = HashMap::new();
    for line in split.lines(Name::Comm).unwrap_or_default() {
        let text = String::from_utf8_lossy(line);
        let Some((pid, rest)) = text.strip_prefix("/proc/").and_then(|rest| rest.split_once('/')) else { continue };
        let (Ok(pid), Some(name)) = (pid.parse::<u32>(), rest.strip_prefix("comm:")) else { continue };
        match names.entry(pid) {
            Entry::Vacant(slot) => {
                slot.insert(sections::text(name.as_bytes(), NAME_LIMIT));
            }
            Entry::Occupied(mut slot) => {
                slot.insert(None);
            }
        }
    }
    Owners { pids, names }
}

fn hex_word(text: &str) -> Option<u32> {
    (text.len() == 8 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(text, 16).ok())
        .flatten()
}

fn address(hex: &str) -> Option<String> {
    match hex.len() {
        8 => Some(Ipv4Addr::from(hex_word(hex)?.to_le_bytes()).to_string()),
        32 => {
            let mut octets = [0u8; 16];
            for (index, chunk) in octets.chunks_mut(4).enumerate() {
                chunk.copy_from_slice(&hex_word(hex.get(index * 8..index * 8 + 8)?)?.to_le_bytes());
            }
            let address = Ipv6Addr::from(octets);
            Some(address.to_ipv4_mapped().map_or_else(|| format!("[{address}]"), |mapped| mapped.to_string()))
        }
        _ => None,
    }
}

fn endpoint(text: &str) -> Option<String> {
    let (hex, port) = text.split_once(':')?;
    let valid_port = port.len() == 4 && port.bytes().all(|byte| byte.is_ascii_hexdigit());
    let port = u16::from_str_radix(port, 16).ok().filter(|_| valid_port)?;
    Some(format!("{}:{port}", address(hex)?))
}

fn state(code: &str) -> Option<SocketState> {
    let valid = code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_hexdigit());
    Some(match u8::from_str_radix(code, 16).ok().filter(|_| valid)? {
        0x01 => SocketState::Established,
        0x02 => SocketState::SynSent,
        0x03 => SocketState::SynRecv,
        0x04 => SocketState::FinWait1,
        0x05 => SocketState::FinWait2,
        0x06 => SocketState::TimeWait,
        0x07 => SocketState::Close,
        0x08 => SocketState::CloseWait,
        0x09 => SocketState::LastAck,
        0x0A => SocketState::Listen,
        0x0B => SocketState::Closing,
        _ => SocketState::Other,
    })
}

fn socket(line: &[u8], protocol: SocketProtocol, owners: &Owners) -> Option<Socket> {
    if line.len() >= MAX_LINE_BYTES {
        return None;
    }
    let text = std::str::from_utf8(line).ok()?;
    let fields: Vec<&str> = text.split_whitespace().take(COLUMNS).collect();
    if fields.len() < COLUMNS || !fields[0].ends_with(':') {
        return None;
    }
    let (local, remote) = (endpoint(fields[LOCAL])?, endpoint(fields[REMOTE])?);
    let state = match (protocol, NO_REMOTE.contains(&remote.as_str())) {
        (SocketProtocol::Udp, true) => SocketState::Listen,
        _ => state(fields[STATE])?,
    };
    let owner = fields[INODE].parse::<u64>().ok().and_then(|inode| owners.of(inode));
    Some(Socket { protocol, local, remote, state, owner })
}

fn has_header(split: &Split, name: Name) -> bool {
    split.lines(name).is_some_and(|lines| lines.iter().any(|line| line.trim_ascii_start().starts_with(b"sl")))
}

pub(super) fn sockets(split: &Split) -> Option<(Vec<Socket>, Vec<Socket>)> {
    if !has_header(split, Name::Tcp) {
        return None;
    }
    let owners = owners(split);
    let mut found = Vec::new();
    for (name, protocol) in [
        (Name::Tcp, SocketProtocol::Tcp),
        (Name::Tcp6, SocketProtocol::Tcp),
        (Name::Udp, SocketProtocol::Udp),
        (Name::Udp6, SocketProtocol::Udp),
    ] {
        let parsed = split.lines(name).unwrap_or_default().iter().filter_map(|line| socket(line, protocol, &owners));
        found.extend(parsed.take(MAX_SOCKETS));
    }
    let (mut listening, mut connections): (Vec<Socket>, Vec<Socket>) =
        found.into_iter().partition(|socket| socket.state == SocketState::Listen);
    listening.truncate(MAX_SOCKETS);
    connections.truncate(MAX_SOCKETS - listening.len());
    Some((listening, connections))
}

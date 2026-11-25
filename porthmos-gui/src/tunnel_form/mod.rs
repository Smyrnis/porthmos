use std::net::IpAddr;

use porthmos_core::{Endpoint, Listen, SpecError, TunnelKey, TunnelKind, TunnelSpec};

pub type DraftError = SpecError;

const LOOPBACK: &str = "127.0.0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftKind {
    Local,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Connection,
    Name,
    ListenHost,
    ListenPort,
    TargetHost,
    TargetPort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftChange {
    Connection(String),
    Name(String),
    Kind(DraftKind),
    ListenHost(String),
    ListenPort(String),
    TargetHost(String),
    TargetPort(String),
    AllowExternal(bool),
    AutoStart(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelDraft {
    pub original: Option<TunnelKey>,
    pub connection: String,
    pub name: String,
    pub kind: DraftKind,
    pub listen_host: String,
    pub listen_port: String,
    pub target_host: String,
    pub target_port: String,
    pub allow_external: bool,
    pub auto_start: bool,
}

fn port_of(text: &str) -> Result<u16, DraftError> {
    match text.trim().parse::<u16>() {
        Ok(0) | Err(_) => Err(SpecError::BadPort),
        Ok(port) => Ok(port),
    }
}

fn probe(change: impl FnOnce(&mut TunnelSpec)) -> Option<DraftError> {
    let mut spec = TunnelSpec {
        connection: "probe".to_string(),
        name: "probe".to_string(),
        kind: TunnelKind::Local {
            listen: Listen { address: IpAddr::from([127, 0, 0, 1]), port: 1 },
            target: Endpoint { host: "probe".to_string(), port: 1 },
        },
        allow_external: false,
        auto_start: false,
    };
    change(&mut spec);
    spec.validate().err()
}

impl TunnelDraft {
    pub fn new(connection: &str) -> Self {
        Self {
            original: None,
            connection: connection.to_string(),
            name: String::new(),
            kind: DraftKind::Local,
            listen_host: LOOPBACK.to_string(),
            listen_port: String::new(),
            target_host: String::new(),
            target_port: String::new(),
            allow_external: false,
            auto_start: false,
        }
    }

    pub fn from_spec(spec: &TunnelSpec) -> Self {
        let listen = spec.kind.listen();
        let (kind, target_host, target_port) = match &spec.kind {
            TunnelKind::Local { target, .. } => (DraftKind::Local, target.host.clone(), target.port.to_string()),
            TunnelKind::Dynamic { .. } => (DraftKind::Dynamic, String::new(), String::new()),
        };
        Self {
            original: Some(spec.key()),
            connection: spec.connection.clone(),
            name: spec.name.clone(),
            kind,
            listen_host: listen.address.to_string(),
            listen_port: listen.port.to_string(),
            target_host,
            target_port,
            allow_external: spec.allow_external,
            auto_start: spec.auto_start,
        }
    }

    pub fn apply(&mut self, change: DraftChange) {
        match change {
            DraftChange::Connection(value) => self.connection = value,
            DraftChange::Name(value) => self.name = value,
            DraftChange::Kind(value) => self.kind = value,
            DraftChange::ListenHost(value) => self.listen_host = value,
            DraftChange::ListenPort(value) => self.listen_port = value,
            DraftChange::TargetHost(value) => self.target_host = value,
            DraftChange::TargetPort(value) => self.target_port = value,
            DraftChange::AllowExternal(value) => self.allow_external = value,
            DraftChange::AutoStart(value) => self.auto_start = value,
        }
    }

    fn listen_address(&self) -> Result<IpAddr, DraftError> {
        let host = self.listen_host.trim();
        if host.is_empty() {
            return Err(SpecError::EmptyHost);
        }
        host.parse::<IpAddr>().map_err(|_| SpecError::BadHost)
    }

    pub fn needs_external_warning(&self) -> bool {
        self.listen_address().is_ok_and(|address| !address.is_loopback())
    }

    pub fn build(&self) -> Result<TunnelSpec, Vec<(Field, DraftError)>> {
        let mut problems = Vec::new();
        let connection = self.connection.trim().to_string();
        let name = self.name.trim().to_string();
        if let Some(error) = probe(|spec| spec.connection = connection.clone()) {
            problems.push((Field::Connection, error));
        }
        if let Some(error) = probe(|spec| spec.name = name.clone()) {
            problems.push((Field::Name, error));
        }
        let address = self.listen_address();
        if let Err(error) = &address {
            problems.push((Field::ListenHost, *error));
        } else if self.needs_external_warning() && !self.allow_external {
            problems.push((Field::ListenHost, SpecError::ExternalNotAllowed));
        }
        let listen_port = port_of(&self.listen_port);
        if let Err(error) = &listen_port {
            problems.push((Field::ListenPort, *error));
        }
        let mut target = None;
        if self.kind == DraftKind::Local {
            let host = self.target_host.trim().to_string();
            if let Some(error) = probe(|spec| {
                spec.kind = TunnelKind::Local {
                    listen: Listen { address: IpAddr::from([127, 0, 0, 1]), port: 1 },
                    target: Endpoint { host: host.clone(), port: 1 },
                }
            }) {
                problems.push((Field::TargetHost, error));
            }
            match port_of(&self.target_port) {
                Ok(port) => target = Some(Endpoint { host, port }),
                Err(error) => problems.push((Field::TargetPort, error)),
            }
        }
        if !problems.is_empty() {
            return Err(problems);
        }
        let (Ok(address), Ok(port)) = (address, listen_port) else {
            return Err(problems);
        };
        let listen = Listen { address, port };
        let kind = match target {
            Some(target) => TunnelKind::Local { listen, target },
            None => TunnelKind::Dynamic { listen },
        };
        Ok(TunnelSpec { connection, name, kind, allow_external: self.allow_external, auto_start: self.auto_start })
    }
}

#[cfg(test)]
mod tests;

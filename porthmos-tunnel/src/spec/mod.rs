use std::{
    fmt,
    net::{IpAddr, Ipv6Addr},
    str::FromStr,
};

const MAX_NAME_CHARS: usize = 64;
const MAX_HOST_BYTES: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecError {
    EmptyName,
    NameTooLong,
    NameHasControl,
    EmptyConnection,
    EmptyHost,
    HostTooLong,
    HostHasControl,
    BadHost,
    BadPort,
    ExternalNotAllowed,
}

impl fmt::Display for SpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            SpecError::EmptyName => "the name is empty",
            SpecError::NameTooLong => "the name is longer than 64 characters",
            SpecError::NameHasControl => "the name contains a control or hidden character",
            SpecError::EmptyConnection => "no connection is named",
            SpecError::EmptyHost => "the host is empty",
            SpecError::HostTooLong => "the host is longer than 255 bytes",
            SpecError::HostHasControl => "the host contains a control character",
            SpecError::BadHost => "the host is not valid",
            SpecError::BadPort => "the port must be a number from 1 to 65535",
            SpecError::ExternalNotAllowed => "listening beyond this computer needs the explicit opt-in",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listen {
    pub address: IpAddr,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Local { listen: Listen, target: Endpoint },
    Dynamic { listen: Listen },
    Remote { listen: Listen, target: Endpoint },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelSpec {
    pub connection: String,
    pub name: String,
    pub kind: Kind,
    pub allow_external: bool,
    pub auto_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TunnelKey {
    pub connection: String,
    pub name: String,
}

fn split_host_port(text: &str, port_zero_allowed: bool) -> Result<(&str, u16), SpecError> {
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        let (inside, after) = rest.split_once(']').ok_or(SpecError::BadHost)?;
        (inside, after.strip_prefix(':').ok_or(SpecError::BadPort)?)
    } else {
        let (host, port) =
            text.rsplit_once(':').ok_or(if text.is_empty() { SpecError::EmptyHost } else { SpecError::BadPort })?;
        if host.contains(':') {
            return Err(SpecError::BadHost);
        }
        (host, port)
    };
    Ok((host, parse_port(port, port_zero_allowed)?))
}

fn parse_port(text: &str, port_zero_allowed: bool) -> Result<u16, SpecError> {
    match text.parse::<u16>() {
        Ok(0) if !port_zero_allowed => Err(SpecError::BadPort),
        Err(_) => Err(SpecError::BadPort),
        Ok(port) => Ok(port),
    }
}

fn check_host(host: &str) -> Result<(), SpecError> {
    if host.is_empty() {
        return Err(SpecError::EmptyHost);
    }
    if host.len() > MAX_HOST_BYTES {
        return Err(SpecError::HostTooLong);
    }
    if host.chars().any(char::is_control) {
        return Err(SpecError::HostHasControl);
    }
    if host.chars().any(char::is_whitespace) || host.contains(['[', ']']) {
        return Err(SpecError::BadHost);
    }
    if host.contains(':') && host.parse::<Ipv6Addr>().is_err() {
        return Err(SpecError::BadHost);
    }
    Ok(())
}

fn hides_text(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}'
        )
}

impl Endpoint {
    pub(crate) fn checked(host: String, port: u16) -> Result<Self, SpecError> {
        if port == 0 {
            return Err(SpecError::BadPort);
        }
        check_host(&host)?;
        Ok(Self { host, port })
    }
}

impl FromStr for Endpoint {
    type Err = SpecError;

    fn from_str(text: &str) -> Result<Self, SpecError> {
        let (host, port) = split_host_port(text, false)?;
        if text.starts_with('[') {
            host.parse::<Ipv6Addr>().map_err(|_| SpecError::BadHost)?;
        } else {
            check_host(host)?;
        }
        Ok(Self { host: host.to_string(), port })
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(formatter, "[{}]:{}", self.host, self.port)
        } else {
            write!(formatter, "{}:{}", self.host, self.port)
        }
    }
}

impl FromStr for Listen {
    type Err = SpecError;

    fn from_str(text: &str) -> Result<Self, SpecError> {
        Self::parse(text, false)
    }
}

impl fmt::Display for Listen {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::net::SocketAddr::new(self.address, self.port).fmt(formatter)
    }
}

impl Listen {
    fn parse(text: &str, port_zero_allowed: bool) -> Result<Self, SpecError> {
        let (host, port) = split_host_port(text, port_zero_allowed)?;
        let address = host.parse::<IpAddr>().map_err(|_| SpecError::BadHost)?;
        Ok(Self { address, port })
    }

    pub fn parse_server_side(text: &str) -> Result<Self, SpecError> {
        Self::parse(text, true)
    }

    pub fn is_loopback(&self) -> bool {
        self.address.is_loopback()
    }
}

impl Kind {
    pub fn listen(&self) -> &Listen {
        match self {
            Kind::Local { listen, .. } | Kind::Dynamic { listen } | Kind::Remote { listen, .. } => listen,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Kind::Local { .. } => "-L",
            Kind::Dynamic { .. } => "-D",
            Kind::Remote { .. } => "-R",
        }
    }
}

impl TunnelSpec {
    pub fn key(&self) -> TunnelKey {
        TunnelKey { connection: self.connection.clone(), name: self.name.clone() }
    }

    pub fn validate(&self) -> Result<(), SpecError> {
        if self.connection.is_empty() {
            return Err(SpecError::EmptyConnection);
        }
        if self.name.is_empty() {
            return Err(SpecError::EmptyName);
        }
        if self.name.chars().count() > MAX_NAME_CHARS {
            return Err(SpecError::NameTooLong);
        }
        if self.name.chars().any(hides_text) {
            return Err(SpecError::NameHasControl);
        }
        let listen = self.kind.listen();
        if listen.port == 0 && !matches!(self.kind, Kind::Remote { .. }) {
            return Err(SpecError::BadPort);
        }
        if let Kind::Local { target, .. } | Kind::Remote { target, .. } = &self.kind {
            if target.port == 0 {
                return Err(SpecError::BadPort);
            }
            check_host(&target.host)?;
        }
        if !listen.is_loopback() && !self.allow_external {
            return Err(SpecError::ExternalNotAllowed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

use std::{
    net::{Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::spec::Endpoint;

const VERSION: u8 = 5;
const NO_AUTH: u8 = 0;
const NO_ACCEPTABLE_METHOD: u8 = 0xff;
const CONNECT: u8 = 1;
const IPV4: u8 = 1;
const DOMAIN: u8 = 3;
const IPV6: u8 = 4;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Connect(Endpoint),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandshakeError {
    BadVersion,
    NoAcceptableMethod,
    UnsupportedCommand,
    UnsupportedAddress,
    Malformed,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reply {
    Succeeded = 0,
    GeneralFailure = 1,
    ConnectionNotAllowed = 2,
    ConnectionRefused = 5,
    CommandNotSupported = 7,
    AddressTypeNotSupported = 8,
}

pub(crate) async fn reply<S: AsyncWrite + Unpin>(stream: &mut S, code: Reply) {
    let _ = stream.write_all(&[VERSION, code as u8, 0, IPV4, 0, 0, 0, 0, 0, 0]).await;
}

pub(crate) async fn handshake<S>(stream: &mut S) -> Result<Request, HandshakeError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    tokio::time::timeout(HANDSHAKE_TIMEOUT, negotiate(stream)).await.unwrap_or(Err(HandshakeError::TimedOut))
}

async fn fill<S: AsyncRead + Unpin>(stream: &mut S, buffer: &mut [u8]) -> Result<(), HandshakeError> {
    stream.read_exact(buffer).await.map(|_| ()).map_err(|_| HandshakeError::Malformed)
}

async fn send<S: AsyncWrite + Unpin>(stream: &mut S, bytes: &[u8]) -> Result<(), HandshakeError> {
    stream.write_all(bytes).await.map_err(|_| HandshakeError::Malformed)
}

async fn negotiate<S>(stream: &mut S) -> Result<Request, HandshakeError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut greeting = [0u8; 2];
    fill(stream, &mut greeting).await?;
    if greeting[0] != VERSION {
        return Err(HandshakeError::BadVersion);
    }
    let mut methods = vec![0u8; usize::from(greeting[1])];
    fill(stream, &mut methods).await?;
    if !methods.contains(&NO_AUTH) {
        send(stream, &[VERSION, NO_ACCEPTABLE_METHOD]).await?;
        return Err(HandshakeError::NoAcceptableMethod);
    }
    send(stream, &[VERSION, NO_AUTH]).await?;

    let mut header = [0u8; 4];
    fill(stream, &mut header).await?;
    let [version, command, reserved, address_type] = header;
    if version != VERSION {
        return Err(HandshakeError::BadVersion);
    }
    if command != CONNECT {
        reply(stream, Reply::CommandNotSupported).await;
        return Err(HandshakeError::UnsupportedCommand);
    }
    if reserved != 0 {
        return Err(HandshakeError::Malformed);
    }
    let host = match address_type {
        IPV4 => {
            let mut octets = [0u8; 4];
            fill(stream, &mut octets).await?;
            Ipv4Addr::from(octets).to_string()
        }
        IPV6 => {
            let mut octets = [0u8; 16];
            fill(stream, &mut octets).await?;
            Ipv6Addr::from(octets).to_string()
        }
        DOMAIN => {
            let mut length = [0u8; 1];
            fill(stream, &mut length).await?;
            let mut name = vec![0u8; usize::from(length[0])];
            fill(stream, &mut name).await?;
            String::from_utf8(name).map_err(|_| HandshakeError::Malformed)?
        }
        _ => {
            reply(stream, Reply::AddressTypeNotSupported).await;
            return Err(HandshakeError::UnsupportedAddress);
        }
    };
    let mut port = [0u8; 2];
    fill(stream, &mut port).await?;
    Endpoint::checked(host, u16::from_be_bytes(port)).map(Request::Connect).map_err(|_| HandshakeError::Malformed)
}

#[cfg(test)]
mod tests;

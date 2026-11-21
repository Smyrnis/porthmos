use std::net::{IpAddr, Ipv4Addr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::Login;

const SOCKS5: u8 = 5;
const SOCKS4: u8 = 4;
const CONNECT: u8 = 1;
const NO_AUTHENTICATION: u8 = 0;
const USER_PASSWORD: u8 = 2;
const LOGIN_VERSION: u8 = 1;
const MAX_FIELD: usize = 255;
const ADDRESS_IPV4: u8 = 1;
const ADDRESS_DOMAIN: u8 = 3;
const ADDRESS_IPV6: u8 = 4;
const GRANTED_V4: u8 = 90;
const PLACEHOLDER_ADDRESS: [u8; 4] = [0, 0, 0, 1];
const LOGIN_NEEDED: &str = "the proxy wants a login: fill in Proxy user and Proxy password";
const LOGIN_REFUSED: &str = "the proxy refused the login (wrong user name or password)";
const METHOD_UNSUPPORTED: &str = "the proxy asked for an authentication method that Porthmos does not support";

fn io_text(error: std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::UnexpectedEof => "the proxy closed the connection during the handshake".to_string(),
        _ => format!("the proxy handshake failed: {error}"),
    }
}

fn failure_v5(code: u8) -> String {
    let reason = match code {
        1 => "general failure",
        2 => "connection not allowed by the proxy",
        3 => "network unreachable",
        4 => "host unreachable",
        5 => "connection refused",
        6 => "timed out",
        7 => "command not supported",
        8 => "address type not supported",
        other => return format!("the proxy refused the connection (code {other})"),
    };
    format!("the proxy refused the connection: {reason}")
}

fn failure_v4(code: u8) -> String {
    match code {
        92 | 93 => "the proxy rejected the request: it checks the user name with identd".to_string(),
        _ => "the proxy rejected the request".to_string(),
    }
}

fn address_v5(host: &str) -> Result<Vec<u8>, String> {
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => Ok([&[ADDRESS_IPV4][..], &address.octets()].concat()),
        Ok(IpAddr::V6(address)) => Ok([&[ADDRESS_IPV6][..], &address.octets()].concat()),
        Err(_) => {
            let length = u8::try_from(host.len()).map_err(|_| "the host name is too long for SOCKS5".to_string())?;
            Ok([&[ADDRESS_DOMAIN, length][..], host.as_bytes()].concat())
        }
    }
}

async fn log_in<S>(stream: &mut S, login: &Login) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let password = login.password.as_deref().unwrap_or_default();
    let mut message = vec![LOGIN_VERSION, login.user.len() as u8];
    message.extend_from_slice(login.user.as_bytes());
    message.push(password.len() as u8);
    message.extend_from_slice(password.as_bytes());
    stream.write_all(&message).await.map_err(io_text)?;
    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).await.map_err(io_text)?;
    if reply[0] != LOGIN_VERSION || reply[1] != 0 {
        return Err(LOGIN_REFUSED.to_string());
    }
    Ok(())
}

pub(crate) async fn socks5<S>(stream: &mut S, host: &str, port: u16, login: Option<&Login>) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let address = address_v5(host)?;
    if login.is_some_and(|login| {
        login.user.len() > MAX_FIELD || login.password.as_deref().is_some_and(|password| password.len() > MAX_FIELD)
    }) {
        return Err("the proxy user or password is too long for SOCKS5 (255 bytes at most)".to_string());
    }
    let greeting: &[u8] = match login {
        Some(_) => &[SOCKS5, 2, NO_AUTHENTICATION, USER_PASSWORD],
        None => &[SOCKS5, 1, NO_AUTHENTICATION],
    };
    stream.write_all(greeting).await.map_err(io_text)?;
    let mut method = [0u8; 2];
    stream.read_exact(&mut method).await.map_err(io_text)?;
    if method[0] != SOCKS5 {
        return Err("the proxy did not answer as a SOCKS5 server (not a SOCKS5 proxy)".to_string());
    }
    match (method[1], login) {
        (NO_AUTHENTICATION, _) => {}
        (USER_PASSWORD, Some(login)) => log_in(stream, login).await?,
        (_, None) => return Err(LOGIN_NEEDED.to_string()),
        (_, Some(_)) => return Err(METHOD_UNSUPPORTED.to_string()),
    }
    let mut request = vec![SOCKS5, CONNECT, 0];
    request.extend_from_slice(&address);
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await.map_err(io_text)?;

    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await.map_err(io_text)?;
    if head[0] != SOCKS5 {
        return Err("the proxy did not answer as a SOCKS5 server (not a SOCKS5 proxy)".to_string());
    }
    if head[1] != 0 {
        return Err(failure_v5(head[1]));
    }
    let bound = match head[3] {
        ADDRESS_IPV4 => 4,
        ADDRESS_IPV6 => 16,
        ADDRESS_DOMAIN => {
            let mut length = [0u8; 1];
            stream.read_exact(&mut length).await.map_err(io_text)?;
            usize::from(length[0])
        }
        _ => return Err("the proxy sent an address type that is not understood".to_string()),
    };
    let mut rest = vec![0u8; bound + 2];
    stream.read_exact(&mut rest).await.map_err(io_text)?;
    Ok(())
}

pub(crate) async fn socks4a<S>(stream: &mut S, host: &str, port: u16, login: Option<&Login>) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut request = vec![SOCKS4, CONNECT];
    request.extend_from_slice(&port.to_be_bytes());
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => request.extend_from_slice(&address.octets()),
        Ok(IpAddr::V6(_)) => return Err("SOCKS4 cannot reach an IPv6 address".to_string()),
        Err(_) => request.extend_from_slice(&PLACEHOLDER_ADDRESS),
    }
    if let Some(login) = login {
        if login.user.contains('\0') || login.user.len() > MAX_FIELD {
            return Err("the proxy user cannot be used with SOCKS4: no NUL bytes and 255 bytes at most".to_string());
        }
        request.extend_from_slice(login.user.as_bytes());
    }
    request.push(0);
    if host.parse::<Ipv4Addr>().is_err() {
        request.extend_from_slice(host.as_bytes());
        request.push(0);
    }
    stream.write_all(&request).await.map_err(io_text)?;

    let mut reply = [0u8; 8];
    stream.read_exact(&mut reply).await.map_err(io_text)?;
    if reply[0] != 0 {
        return Err("the proxy did not answer as a SOCKS4 server (not a SOCKS4 proxy)".to_string());
    }
    if reply[1] != GRANTED_V4 {
        return Err(failure_v4(reply[1]));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

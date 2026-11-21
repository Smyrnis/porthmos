use std::net::IpAddr;

use base64::{Engine, engine::general_purpose::STANDARD};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::Login;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_STATUS_TEXT: usize = 120;
const HEADER_END: &[u8] = b"\r\n\r\n";

fn authority(host: &str, port: u16) -> Result<String, String> {
    if host.is_empty() || host.chars().any(|character| character.is_control() || character.is_whitespace()) {
        return Err("the host name has characters that cannot go into an HTTP request".to_string());
    }
    Ok(match host.parse::<IpAddr>() {
        Ok(IpAddr::V6(address)) => format!("[{address}]:{port}"),
        _ => format!("{host}:{port}"),
    })
}

fn printable(text: &str) -> String {
    text.chars().filter(|character| !character.is_control()).take(MAX_STATUS_TEXT).collect()
}

async fn read_head<S>(stream: &mut S) -> Result<Vec<u8>, String>
where
    S: AsyncRead + Unpin,
{
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(HEADER_END) {
        if head.len() >= MAX_HEADER_BYTES {
            return Err("the proxy answer is too long to be an HTTP answer".to_string());
        }
        match stream.read_exact(&mut byte).await {
            Ok(_) => head.push(byte[0]),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err("the proxy closed the connection during the handshake".to_string());
            }
            Err(error) => return Err(format!("the proxy handshake failed: {error}")),
        }
    }
    Ok(head)
}

fn authorization(login: Option<&Login>) -> Result<String, String> {
    let Some(login) = login else {
        return Ok(String::new());
    };
    if login.user.contains(':') {
        return Err("the proxy user cannot contain a colon: HTTP proxies split the login at it".to_string());
    }
    let credentials = format!("{}:{}", login.user, login.password.as_deref().unwrap_or_default());
    Ok(format!("Proxy-Authorization: Basic {}\r\n", STANDARD.encode(credentials)))
}

pub(crate) async fn connect<S>(stream: &mut S, host: &str, port: u16, login: Option<&Login>) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let authority = authority(host, port)?;
    let authorization = authorization(login)?;
    let request = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n{authorization}\r\n");
    stream.write_all(request.as_bytes()).await.map_err(|error| format!("the proxy handshake failed: {error}"))?;

    let head = read_head(stream).await?;
    let text = String::from_utf8_lossy(&head);
    let status_line = text.lines().next().unwrap_or_default();
    let mut parts = status_line.splitn(3, ' ');
    let (version, code, reason) = (parts.next().unwrap_or_default(), parts.next(), parts.next().unwrap_or_default());
    let Some(code) = code.filter(|_| version.starts_with("HTTP/1.")) else {
        return Err("the answer is not an HTTP answer (not an HTTP proxy)".to_string());
    };
    match code.parse::<u16>() {
        Ok(200..=299) => Ok(()),
        Ok(407) if login.is_some() => Err(format!(
            "the proxy answered 407 {}: it refused the login (wrong user name or password)",
            printable(reason)
        )),
        Ok(407) => Err(format!(
            "the proxy answered 407 {}: it wants a login; fill in Proxy user and Proxy password",
            printable(reason)
        )),
        Ok(status) => Err(format!("the proxy refused the connection: {status} {}", printable(reason))),
        Err(_) => Err("the answer is not an HTTP answer (not an HTTP proxy)".to_string()),
    }
}

#[cfg(test)]
mod tests;

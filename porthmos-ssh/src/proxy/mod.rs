mod command;
mod http;
mod socks;

pub(crate) use command::{Local, Tokens};

use std::time::Duration;

use porthmos_vfs::Target;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    jump::{effective_jump, host_and_port, matching_alias},
    ssh_config::SshConfigHost,
};

const DEFAULT_HTTP_PORT: u16 = 8080;
const DEFAULT_SOCKS_PORT: u16 = 1080;
const HANDSHAKE_TIME: Duration = Duration::from_secs(30);
const PROXY_CONNECT_TIME: Duration = Duration::from_secs(30);

pub(crate) trait Transport: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Transport for T {}

pub(crate) struct Opened {
    pub(crate) stream: Box<dyn Transport>,
    pub(crate) diagnostics: Option<command::Diagnostics>,
}

pub(crate) async fn explain(diagnostics: Option<command::Diagnostics>, error: anyhow::Error) -> anyhow::Error {
    let Some(diagnostics) = diagnostics else {
        return error;
    };
    match diagnostics.text().await {
        text if text.is_empty() => error,
        text => error.context(format!("the proxy command said: {text}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProxyKind {
    Http,
    Socks5,
    Socks4,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Login {
    pub(crate) user: String,
    pub(crate) password: Option<String>,
}

impl std::fmt::Debug for Login {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Login")
            .field("user", &self.user)
            .field("password", &self.password.as_ref().map(|_| "<hidden>"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Route {
    Command(String),
    Proxy { kind: ProxyKind, host: String, port: u16, login: Option<Login> },
}

fn clean(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty() && !text.eq_ignore_ascii_case("none"))
}

pub(crate) fn parse_proxy(text: &str) -> Result<Route, String> {
    let text = text.trim();
    let Some((scheme, rest)) = text.split_once("://") else {
        return Err("write the proxy as http://host:port, socks5://host:port or socks4://host:port".to_string());
    };
    let (kind, default_port) = match scheme.to_ascii_lowercase().as_str() {
        "http" => (ProxyKind::Http, DEFAULT_HTTP_PORT),
        "socks5" | "socks5h" => (ProxyKind::Socks5, DEFAULT_SOCKS_PORT),
        "socks4" | "socks4a" => (ProxyKind::Socks4, DEFAULT_SOCKS_PORT),
        other => return Err(format!("the proxy type \"{other}\" is not supported; use http, socks5 or socks4")),
    };
    if rest.contains('@') {
        return Err(
            "leave the user name and password out of the address and fill in Proxy user and Proxy password".to_string()
        );
    }
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.contains('/') {
        return Err("the proxy address has a path; write only host and port".to_string());
    }
    let (host, port) = host_and_port(rest)?;
    if host.is_empty() {
        return Err("the proxy address has no host".to_string());
    }
    Ok(Route::Proxy { kind, host, port: port.unwrap_or(default_port), login: None })
}

pub(crate) fn declared_command(target: &Target) -> Option<&str> {
    clean(target.option("proxy_command"))
}

pub(crate) fn declared_proxy(target: &Target) -> Option<&str> {
    clean(target.option("proxy"))
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.trim().is_empty())
}

fn declared_login(target: &Target) -> (Option<&str>, Option<&str>) {
    (
        non_empty(target.option("proxy_user")).map(str::trim),
        target.option("proxy_password").filter(|text| !text.is_empty()),
    )
}

fn proxy_with_login(target: &Target, proxy: &str) -> Result<Route, String> {
    let Route::Proxy { kind, host, port, .. } =
        parse_proxy(proxy).map_err(|reason| format!("Can't use the proxy setting: {reason}"))?
    else {
        unreachable!("an address always parses to a proxy route");
    };
    let login = match declared_login(target) {
        (None, None) => None,
        (None, Some(_)) => return Err("Can't use the proxy settings: a Proxy password needs a Proxy user".to_string()),
        (Some(_), Some(_)) if kind == ProxyKind::Socks4 => {
            return Err("Can't use the proxy settings: SOCKS4 has no password; use socks5 or http".to_string());
        }
        (Some(user), password) => Some(Login { user: user.to_string(), password: password.map(str::to_string) }),
    };
    Ok(Route::Proxy { kind, host, port, login })
}

pub(crate) fn route_for(target: &Target, hosts: &[SshConfigHost]) -> Result<Option<Route>, String> {
    let command = clean(target.option("proxy_command"));
    let proxy = clean(target.option("proxy"));
    let switched_off = target.option("proxy").is_some_and(|setting| setting.trim().eq_ignore_ascii_case("none"));
    if proxy.is_none() && !switched_off && declared_login(target) != (None, None) {
        return Err(
            "Can't use the proxy settings: Proxy user and Proxy password only work together with Proxy".to_string()
        );
    }
    match (command, proxy) {
        (Some(_), Some(_)) => {
            Err("Can't use the proxy settings: set either Proxy or Proxy command, not both".to_string())
        }
        (Some(command), None) => Ok(Some(Route::Command(command.to_string()))),
        (None, Some(proxy)) => proxy_with_login(target, proxy).map(Some),
        (None, None) if target.option("proxy_command").is_some() => Ok(None),
        (None, None) if effective_jump(target, hosts).is_some() => Ok(None),
        (None, None) => Ok(matching_alias(target, hosts)
            .and_then(|alias| clean(alias.proxy_command.as_deref()))
            .map(|command| Route::Command(command.to_string()))),
    }
}

const FAIL_CLOSED: &str = "false";

fn netcat_command(kind: ProxyKind, host: &str, port: u16) -> Option<String> {
    if !command::safe_in_a_command(host, &['.', '-', '_', ':']) {
        return None;
    }
    let flag = match kind {
        ProxyKind::Http => "connect",
        ProxyKind::Socks5 => "5",
        ProxyKind::Socks4 => "4",
    };
    let address = if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
    Some(guarded(&format!("-X {flag} -x {address}")))
}

#[cfg(unix)]
fn guarded(options: &str) -> String {
    format!(
        "sh -c 'nc -h 2>&1 | grep -q proxy_protocol || {{ echo \"Porthmos: this proxy needs the OpenBSD version of nc \
         (netcat-openbsd). Install it, or use Proxy command instead.\" >&2; exit 1; }}; exec nc {options} \"$1\" \"$2\"' sh %h %p"
    )
}

#[cfg(not(unix))]
fn guarded(options: &str) -> String {
    format!("nc {options} %h %p")
}

pub(crate) fn terminal_command(target: &Target) -> Option<String> {
    let command = clean(target.option("proxy_command"));
    let proxy = clean(target.option("proxy"));
    match (command, proxy) {
        (None, None) => target
            .option("proxy_command")
            .filter(|setting| setting.trim().eq_ignore_ascii_case("none"))
            .map(|_| "none".to_string()),
        (Some(command), None) => Some(command.to_string()),
        (Some(_), Some(_)) => Some(FAIL_CLOSED.to_string()),
        (None, Some(_)) if declared_login(target) != (None, None) => Some(FAIL_CLOSED.to_string()),
        (None, Some(proxy)) => Some(match parse_proxy(proxy) {
            Ok(Route::Proxy { kind, host, port, .. }) => {
                netcat_command(kind, &host, port).unwrap_or_else(|| FAIL_CLOSED.to_string())
            }
            _ => FAIL_CLOSED.to_string(),
        }),
    }
}

async fn handshake<S>(
    kind: ProxyKind, stream: &mut S, host: &str, port: u16, login: Option<&Login>, limit: Duration,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let talking = async {
        match kind {
            ProxyKind::Http => http::connect(stream, host, port, login).await,
            ProxyKind::Socks5 => socks::socks5(stream, host, port, login).await,
            ProxyKind::Socks4 => socks::socks4a(stream, host, port, login).await,
        }
    };
    tokio::time::timeout(limit, talking).await.unwrap_or_else(|_| Err("the proxy did not answer in time".to_string()))
}

async fn through_proxy(
    kind: ProxyKind, proxy: (&str, u16), login: Option<&Login>, host: &str, port: u16,
) -> Result<Opened, String> {
    let (proxy_host, proxy_port) = proxy;
    let reaching = tokio::net::TcpStream::connect((proxy_host, proxy_port));
    let mut stream = tokio::time::timeout(PROXY_CONNECT_TIME, reaching)
        .await
        .map_err(|_| format!("could not reach the proxy at {proxy_host}:{proxy_port}: it did not answer in time"))?
        .map_err(|error| format!("could not reach the proxy at {proxy_host}:{proxy_port}: {error}"))?;
    let _ = stream.set_nodelay(true);
    handshake(kind, &mut stream, host, port, login, HANDSHAKE_TIME).await?;
    Ok(Opened { stream: Box::new(stream), diagnostics: None })
}

pub(crate) async fn open_route(route: &Route, tokens: &Tokens<'_>) -> Result<Opened, String> {
    let (host, port) = (tokens.host, tokens.port);
    match route {
        Route::Command(command) => {
            let expanded = command::expand(command, tokens)?;
            let (stream, diagnostics) =
                command::spawn(&expanded).map_err(|error| format!("could not start the proxy command: {error}"))?;
            Ok(Opened { stream: Box::new(stream), diagnostics: Some(diagnostics) })
        }
        Route::Proxy { kind, host: proxy_host, port: proxy_port, login } => {
            through_proxy(*kind, (proxy_host, *proxy_port), login.as_ref(), host, port).await
        }
    }
}

#[cfg(test)]
mod tests;

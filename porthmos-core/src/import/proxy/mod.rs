use std::{collections::BTreeMap, net::Ipv6Addr};

use super::clean;

const NOT_IMPORTED_TYPE: &str =
    "the proxy of this session is not imported: only SOCKS4, SOCKS5 and HTTP proxies can be imported";
const PASSWORD_NOT_IMPORTED: &str = "the proxy password is not imported: type it into Proxy password";
const NOT_IMPORTED_ADDRESS: &str = "the proxy of this session is not imported: its address is not valid";

#[derive(Default)]
pub(crate) struct ProxyFields {
    pub(crate) method: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) port: Option<String>,
    pub(crate) user: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Imported {
    pub(crate) options: BTreeMap<String, String>,
    pub(crate) notes: Vec<String>,
}

fn scheme_of(method: &str) -> Option<&'static str> {
    match method {
        "1" => Some("socks4"),
        "2" => Some("socks5"),
        "3" => Some("http"),
        _ => None,
    }
}

fn host_part(raw: &str) -> Option<String> {
    let host = raw.trim();
    let plain = !host.is_empty()
        && !host.chars().any(|character| {
            character.is_control() || character.is_whitespace() || matches!(character, '@' | '/' | '#' | '[' | ']')
        });
    if !plain {
        return None;
    }
    if host.contains(':') {
        return host.parse::<Ipv6Addr>().ok().map(|_| format!("[{host}]"));
    }
    Some(host.to_string())
}

fn port_part(raw: Option<&str>) -> Result<String, ()> {
    match raw.map(str::trim) {
        None | Some("") | Some("0") => Ok(String::new()),
        Some(text) => text.parse::<u16>().map(|port| format!(":{port}")).map_err(|_| ()),
    }
}

pub(crate) fn proxy_options(fields: &ProxyFields) -> Imported {
    let mut imported = Imported::default();
    let method = fields.method.as_deref().map(str::trim).unwrap_or_default();
    if method.is_empty() || method == "0" {
        return imported;
    }
    let Some(scheme) = scheme_of(method) else {
        imported.notes.push(NOT_IMPORTED_TYPE.to_string());
        return imported;
    };
    let host = fields.host.as_deref().and_then(host_part);
    let port = port_part(fields.port.as_deref());
    let (Some(host), Ok(port)) = (host, port) else {
        imported.notes.push(NOT_IMPORTED_ADDRESS.to_string());
        return imported;
    };
    imported.options.insert("proxy".to_string(), format!("{scheme}://{host}{port}"));
    let user = clean::printable(fields.user.as_deref().unwrap_or_default().trim()).trim().to_string();
    if !user.is_empty() {
        imported.options.insert("proxy_user".to_string(), user);
        imported.notes.push(PASSWORD_NOT_IMPORTED.to_string());
    }
    imported
}

#[cfg(test)]
mod tests;

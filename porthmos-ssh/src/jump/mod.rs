use std::collections::BTreeMap;

use porthmos_vfs::Target;

use crate::ssh_config::SshConfigHost;

pub(crate) const MAX_HOPS: usize = 8;
const DEFAULT_PORT: u16 = 22;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JumpHop {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

pub(crate) fn declared_jump(target: &Target) -> Option<&str> {
    let spec = target.option("proxy_jump")?.trim();
    (!spec.is_empty() && !spec.eq_ignore_ascii_case("none")).then_some(spec)
}

pub(crate) fn matching_alias<'a>(target: &Target, hosts: &'a [SshConfigHost]) -> Option<&'a SshConfigHost> {
    hosts.iter().find(|alias| {
        alias.name.eq_ignore_ascii_case(&target.name)
            && alias.host_name.as_deref().unwrap_or(&alias.name) == target.host
            && alias.port.unwrap_or(DEFAULT_PORT) == target.port
    })
}

pub(crate) fn effective_jump<'a>(target: &'a Target, hosts: &'a [SshConfigHost]) -> Option<&'a str> {
    if target.option("proxy_jump").is_some() {
        return declared_jump(target);
    }
    matching_alias(target, hosts).and_then(|alias| alias.proxy_jump.as_deref())
}

pub(crate) fn parse_port(text: &str) -> Result<u16, String> {
    match text.parse::<u16>() {
        Ok(0) | Err(_) => Err(format!("\"{text}\" is not a valid port")),
        Ok(port) => Ok(port),
    }
}

pub(crate) fn host_and_port(text: &str) -> Result<(String, Option<u16>), String> {
    if let Some(rest) = text.strip_prefix('[') {
        let (host, after) = rest.split_once(']').ok_or_else(|| format!("\"{text}\" is missing a closing bracket"))?;
        let port = match after.strip_prefix(':') {
            Some(port) => Some(parse_port(port)?),
            None if after.is_empty() => None,
            None => return Err(format!("\"{text}\" has unexpected text after the bracket")),
        };
        return Ok((host.to_string(), port));
    }
    match text.split_once(':') {
        Some((host, port)) if !port.contains(':') => Ok((host.to_string(), Some(parse_port(port)?))),
        _ => Ok((text.to_string(), None)),
    }
}

fn parse_hop(text: &str) -> Result<JumpHop, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("a jump host is empty".to_string());
    }
    let text = text.strip_prefix("ssh://").unwrap_or(text);
    let (user, rest) = match text.rsplit_once('@') {
        Some((user, rest)) => (Some(user), rest),
        None => (None, text),
    };
    if user.is_some_and(str::is_empty) {
        return Err(format!("\"{text}\" has no user before the @"));
    }
    let (host, port) = host_and_port(rest)?;
    if host.is_empty() {
        return Err(format!("\"{text}\" has no host"));
    }
    Ok(JumpHop { user: user.map(str::to_string), host, port })
}

pub(crate) fn parse_hops(spec: &str) -> Result<Vec<JumpHop>, String> {
    spec.split(',').map(parse_hop).collect()
}

fn find_alias<'a>(hosts: &'a [SshConfigHost], name: &str) -> Option<&'a SshConfigHost> {
    hosts.iter().find(|host| host.name.eq_ignore_ascii_case(name))
}

fn target_of(hop: &JumpHop, alias: Option<&SshConfigHost>, fallback_user: &str) -> Target {
    let mut options = BTreeMap::new();
    if let Some(identity_file) = alias.and_then(|alias| alias.identity_file.as_ref()) {
        options.insert("identity_file".to_string(), identity_file.to_string_lossy().into_owned());
    }
    Target {
        name: hop.host.clone(),
        host: alias.and_then(|alias| alias.host_name.clone()).unwrap_or_else(|| hop.host.clone()),
        port: hop.port.or_else(|| alias.and_then(|alias| alias.port)).unwrap_or(DEFAULT_PORT),
        username: hop
            .user
            .clone()
            .or_else(|| alias.and_then(|alias| alias.user.clone()))
            .unwrap_or_else(|| fallback_user.to_string()),
        password: None,
        options,
    }
}

fn expand(
    spec: &str, hosts: &[SshConfigHost], fallback_user: &str, depth: usize, chain: &mut Vec<Target>,
) -> Result<(), String> {
    if depth > MAX_HOPS {
        return Err("the jump hosts loop back on themselves or are chained too deeply".to_string());
    }
    for hop in parse_hops(spec)? {
        let alias = find_alias(hosts, &hop.host);
        if let Some(inner) = alias.and_then(|alias| alias.proxy_jump.as_deref()) {
            expand(inner, hosts, fallback_user, depth + 1, chain)?;
        }
        chain.push(target_of(&hop, alias, fallback_user));
        if chain.len() > MAX_HOPS {
            return Err(format!("more than {MAX_HOPS} jump hosts is too many"));
        }
    }
    Ok(())
}

pub(crate) fn resolve_chain(spec: &str, hosts: &[SshConfigHost], fallback_user: &str) -> Result<Vec<Target>, String> {
    let mut chain = Vec::new();
    expand(spec, hosts, fallback_user, 0, &mut chain)?;
    Ok(chain)
}

#[cfg(test)]
mod tests;

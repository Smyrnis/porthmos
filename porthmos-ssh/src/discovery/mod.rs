use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use porthmos_vfs::{Environment, ProtocolError, ShellInvocation, Target};

use crate::{
    jump::{declared_jump, effective_jump},
    proxy::{declared_command, declared_proxy},
    shell, ssh_config,
    version::installed_ssh_version,
};

pub const DEFAULT_PORT: u16 = 22;
pub(crate) const FALLBACK_USERNAME: &str = "root";

fn discovered_target(host: ssh_config::SshConfigHost, env: &Environment) -> Target {
    let mut options = BTreeMap::new();
    if let Some(identity_file) = host.identity_file {
        options.insert("identity_file".to_string(), identity_file.to_string_lossy().into_owned());
    }
    if let Some(proxy_jump) = host.proxy_jump {
        options.insert("proxy_jump".to_string(), proxy_jump);
    }
    if let Some(proxy_command) = host.proxy_command {
        options.insert("proxy_command".to_string(), proxy_command);
    }
    Target {
        host: host.host_name.unwrap_or_else(|| host.name.clone()),
        name: host.name,
        port: host.port.unwrap_or(DEFAULT_PORT),
        username: host.user.or_else(|| env.user.clone()).unwrap_or_else(|| FALLBACK_USERNAME.to_string()),
        password: None,
        options,
    }
}

fn matches_its_ssh_config_alias(target: &Target, env: &Environment) -> bool {
    let Ok(hosts) = ssh_config::load(env.home.as_deref()) else {
        return false;
    };
    hosts.into_iter().filter(|host| host.name == target.name).map(|host| discovered_target(host, env)).any(
        |configured| {
            configured.host == target.host
                && configured.port == target.port
                && configured.username == target.username
                && configured.option("identity_file") == target.option("identity_file")
                && (target.option("proxy_jump").is_none() || declared_jump(&configured) == declared_jump(target))
                && (target.option("proxy_command").is_none()
                    || declared_command(&configured) == declared_command(target))
                && declared_proxy(target).is_none()
        },
    )
}

pub fn identity_path(path: &str, home: Option<&Path>) -> PathBuf {
    match (path.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home.to_path_buf(),
        (Some(rest), Some(home)) if rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(path),
    }
}

pub fn discover(env: &Environment) -> Result<Vec<Target>, ProtocolError> {
    let hosts = ssh_config::load(env.home.as_deref())?;
    Ok(hosts.into_iter().map(|host| discovered_target(host, env)).collect())
}

fn with_inherited_jump(target: &Target, env: &Environment) -> Target {
    let mut effective = target.clone();
    let proxied = target.option("proxy").is_some() || target.option("proxy_command").is_some();
    if proxied && target.option("proxy_jump").is_none() {
        let hosts = ssh_config::load(env.home.as_deref()).unwrap_or_default();
        if let Some(jump) = effective_jump(target, &hosts) {
            effective.options.insert("proxy_jump".to_string(), jump.to_string());
        }
    }
    effective
}

pub fn shell_command(target: &Target, env: &Environment) -> ShellInvocation {
    let sshpass = env.path.as_deref().and_then(shell::find_sshpass_in);
    let helper = shell::live_helper(std::env::current_exe().ok());
    let version = target.password.as_ref().and_then(|_| installed_ssh_version());
    let login = shell::password_login(target.password.as_deref(), helper, version, sshpass);
    let alias = matches_its_ssh_config_alias(target, env).then_some(target.name.as_str());
    let target = &with_inherited_jump(target, env);
    shell::invocation_for(target, alias, login)
}

#[cfg(test)]
mod tests;

use std::{future::Future, path::PathBuf};

use anyhow::anyhow;
use porthmos_vfs::{Answer, Environment, ErrorKind, Prompter, ProtocolError, Question, Target};
use russh::client::Handle;
use tokio::sync::mpsc;

use crate::{
    client::{self, HostKeyCheck, HostKeyDeclined, HostKeyQuestion, PorthmosHandler, ServerId},
    discovery::{FALLBACK_USERNAME, identity_path},
    jump::{effective_jump, resolve_chain},
    proxy::{self, Route},
    ssh_config::{self, SshConfigHost},
};

#[derive(Debug, Clone)]
pub struct ConnectOptions {
    pub known_hosts: Option<PathBuf>,
    pub use_agent: bool,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self { known_hosts: None, use_agent: true }
    }
}

pub struct Session {
    handle: Handle<PorthmosHandler>,
    server: ServerId,
    _jumps: Vec<Handle<PorthmosHandler>>,
}

impl Session {
    pub fn handle(&self) -> &Handle<PorthmosHandler> {
        &self.handle
    }

    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    pub fn server_version(&self) -> Option<String> {
        self.server.version()
    }
}

pub(crate) fn connect_error(err: anyhow::Error) -> ProtocolError {
    if err.downcast_ref::<HostKeyDeclined>().is_some() {
        return ProtocolError::new(ErrorKind::Cancelled, anyhow!("Connection cancelled"));
    }
    ProtocolError::new(ErrorKind::Connect, err)
}

pub(crate) async fn answer_host_key_questions<T>(
    connecting: impl Future<Output = T>, asked: &mut mpsc::Receiver<HostKeyQuestion>, prompter: &mut dyn Prompter,
    target: &Target,
) -> T {
    tokio::pin!(connecting);
    loop {
        tokio::select! {
            result = &mut connecting => return result,
            Some(question) = asked.recv() => {
                let HostKeyQuestion { key_type, fingerprint, reply } = question;
                let trust = Question::TrustHostKey {
                    name: target.name.clone(),
                    host: target.host.clone(),
                    port: target.port,
                    key_type,
                    fingerprint,
                };
                let trusted = matches!(prompter.ask(trust).await, Some(Answer::Confirmed));
                let _ = reply.send(trusted);
            }
        }
    }
}

struct Via<'a> {
    handle: &'a Handle<PorthmosHandler>,
    name: &'a str,
}

async fn reach_directly_or_by_proxy(
    target: &Target, route: Option<&Route>, local_user: &str, host_key: HostKeyCheck,
) -> anyhow::Result<(Handle<PorthmosHandler>, ServerId)> {
    let Some(route) = route else {
        return client::connect(host_key).await;
    };
    let local = proxy::Local::detect(local_user);
    let tokens = proxy::Tokens {
        host: &target.host,
        port: target.port,
        user: &target.username,
        name: &target.name,
        local: &local,
    };
    let opened =
        proxy::open_route(route, &tokens).await.map_err(|reason| anyhow!("Couldn't use the proxy: {reason}"))?;
    let diagnostics = opened.diagnostics.clone();
    match client::connect_over(opened.stream, host_key).await {
        Err(error) if error.downcast_ref::<HostKeyDeclined>().is_none() => {
            Err(proxy::explain(diagnostics, error).await)
        }
        other => other,
    }
}

async fn open(
    target: &Target, via: Option<Via<'_>>, route: Option<&Route>, local_user: &str, prompter: &mut dyn Prompter,
    options: &ConnectOptions,
) -> Result<(Handle<PorthmosHandler>, ServerId), ProtocolError> {
    let (questions, mut asked) = mpsc::channel(1);
    let known_hosts = options.known_hosts.clone().or_else(client::default_known_hosts);
    let host_key = HostKeyCheck::new(&target.host, target.port, known_hosts, questions);
    let connecting = async {
        match via {
            Some(Via { handle, name }) => {
                let channel = handle
                    .channel_open_direct_tcpip(target.host.clone(), u32::from(target.port), "127.0.0.1", 0)
                    .await
                    .map_err(|err| {
                        anyhow!("the jump host {name} refused to forward to {}:{}: {err}", target.host, target.port)
                    })?;
                client::connect_over(channel.into_stream(), host_key).await
            }
            None => reach_directly_or_by_proxy(target, route, local_user, host_key).await,
        }
    };
    let (mut handle, server) =
        answer_host_key_questions(connecting, &mut asked, prompter, target).await.map_err(connect_error)?;

    let home = std::env::home_dir();
    let identity_file = target.option("identity_file").map(|path| identity_path(path, home.as_deref()));
    match client::authenticate_non_interactive(
        &mut handle,
        &target.username,
        identity_file.as_deref(),
        target.password.as_deref(),
        options.use_agent,
    )
    .await
    {
        Ok(true) => return Ok((handle, server)),
        Ok(false) => {}
        Err(err) => return Err(ProtocolError::new(ErrorKind::Auth, err)),
    }

    let question = Question::Password {
        username: target.username.clone(),
        name: target.name.clone(),
        secret: porthmos_vfs::PASSWORD_LABEL,
    };
    let Some(Answer::Password(password)) = prompter.ask(question).await else {
        return Err(ProtocolError::new(ErrorKind::Cancelled, anyhow!("Connection cancelled")));
    };

    match client::authenticate_password(&mut handle, &target.username, &password).await {
        Ok(true) => Ok((handle, server)),
        Ok(false) => {
            Err(ProtocolError::new(ErrorKind::AuthRejected, anyhow!("Authentication failed for {}", target.name)))
        }
        Err(err) => Err(ProtocolError::new(ErrorKind::Auth, err)),
    }
}

fn jump_targets(target: &Target, hosts: &[SshConfigHost], fallback_user: &str) -> Result<Vec<Target>, ProtocolError> {
    let Some(spec) = effective_jump(target, hosts) else {
        return Ok(Vec::new());
    };
    resolve_chain(spec, hosts, fallback_user).map_err(|reason| {
        ProtocolError::new(ErrorKind::Connect, anyhow!("Can't use the jump host setting \"{spec}\": {reason}"))
    })
}

fn reaching_jump(name: &str, error: ProtocolError) -> ProtocolError {
    match error.kind() {
        ErrorKind::Connect => {
            ProtocolError::new(ErrorKind::Connect, anyhow!("Couldn't reach jump host {name}: {error}"))
        }
        _ => error,
    }
}

async fn connect_through(
    target: &Target, prompter: &mut dyn Prompter, options: &ConnectOptions, hosts: &[SshConfigHost],
    fallback_user: &str,
) -> Result<Session, ProtocolError> {
    let hops = jump_targets(target, hosts, fallback_user)?;
    let refused = |reason: String| ProtocolError::new(ErrorKind::Connect, anyhow!(reason));
    let target_route = proxy::route_for(target, hosts).map_err(refused)?;
    let first_route = match (target_route, hops.first()) {
        (Some(explicit), _) => Some(explicit),
        (None, Some(first_hop)) => proxy::route_for(first_hop, hosts).map_err(refused)?,
        (None, None) => None,
    };
    let mut jumps: Vec<Handle<PorthmosHandler>> = Vec::new();
    for hop in &hops {
        let via = jumps
            .last()
            .zip(jumps.len().checked_sub(1).and_then(|index| hops.get(index)))
            .map(|(handle, previous)| Via { handle, name: previous.name.as_str() });
        let route = if jumps.is_empty() { first_route.as_ref() } else { None };
        let (handle, _) = open(hop, via, route, fallback_user, prompter, options)
            .await
            .map_err(|error| reaching_jump(&hop.name, error))?;
        jumps.push(handle);
    }
    let via = jumps.last().zip(hops.last()).map(|(handle, last)| Via { handle, name: last.name.as_str() });
    let route = if hops.is_empty() { first_route.as_ref() } else { None };
    let (handle, server) = open(target, via, route, fallback_user, prompter, options).await?;
    Ok(Session { handle, server, _jumps: jumps })
}

pub async fn connect(
    target: &Target, prompter: &mut dyn Prompter, options: &ConnectOptions,
) -> Result<Session, ProtocolError> {
    let environment = Environment::from_process();
    let hosts = ssh_config::load(environment.home.as_deref()).unwrap_or_default();
    let fallback_user = environment.user.unwrap_or_else(|| FALLBACK_USERNAME.to_string());
    connect_through(target, prompter, options, &hosts, &fallback_user).await
}

#[cfg(test)]
mod tests;

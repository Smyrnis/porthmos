use std::{
    fmt,
    fs::DirBuilder,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use anyhow::{Result, anyhow};
use porthmos_vfs::{SERVER_INFO_CELLS, clean_server_text};
use russh::{
    Signer,
    client::{self, AuthResult, Handle},
    keys,
    keys::{
        HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate,
        agent::{AgentIdentity, client::AgentClient},
    },
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::{mpsc, oneshot},
};

const SNIFF_LIMIT: usize = 512;

#[derive(Debug, PartialEq, Eq)]
pub enum HostKeyStatus {
    Known,
    Unknown,
    Changed { line: usize },
}

pub fn host_key_status(known_hosts: &Path, host: &str, port: u16, key: &PublicKey) -> Result<HostKeyStatus> {
    match keys::check_known_hosts_path(host, port, key, known_hosts) {
        Ok(true) => Ok(HostKeyStatus::Known),
        Ok(false) => Ok(HostKeyStatus::Unknown),
        Err(keys::Error::KeyChanged { line }) => {
            Ok(HostKeyStatus::Changed { line: line_in_file(known_hosts, line).unwrap_or(line) })
        }
        Err(err) => Err(err.into()),
    }
}

fn line_in_file(known_hosts: &Path, uncommented_line: usize) -> Option<usize> {
    let contents = std::fs::read_to_string(known_hosts).ok()?;
    contents
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.starts_with('#'))
        .nth(uncommented_line.checked_sub(1)?)
        .map(|(index, _)| index + 1)
}

pub fn trust_host_key(known_hosts: &Path, host: &str, port: u16, key: &PublicKey) -> Result<()> {
    if let Some(ssh_dir) = known_hosts.parent() {
        DirBuilder::new().recursive(true).mode(0o700).create(ssh_dir)?;
    }
    keys::known_hosts::learn_known_hosts_path(host, port, key, known_hosts)?;
    Ok(())
}

pub fn default_known_hosts() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".ssh").join("known_hosts"))
}

#[derive(Debug)]
pub struct HostKeyDeclined;

impl fmt::Display for HostKeyDeclined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the host key was not trusted")
    }
}

impl std::error::Error for HostKeyDeclined {}

pub struct HostKeyQuestion {
    pub key_type: String,
    pub fingerprint: String,
    pub reply: oneshot::Sender<bool>,
}

pub struct HostKeyCheck {
    host: String,
    port: u16,
    known_hosts: Option<PathBuf>,
    questions: mpsc::Sender<HostKeyQuestion>,
}

impl HostKeyCheck {
    pub fn new(host: &str, port: u16, known_hosts: Option<PathBuf>, questions: mpsc::Sender<HostKeyQuestion>) -> Self {
        Self { host: host.to_string(), port, known_hosts, questions }
    }

    pub async fn verify(&self, key: &PublicKey) -> Result<bool> {
        let Some(known_hosts) = self.known_hosts.as_deref() else {
            return Err(anyhow!(
                "can't verify the host key for {}:{} \u{2014} no home directory to find ~/.ssh/known_hosts in",
                self.host,
                self.port
            ));
        };

        match host_key_status(known_hosts, &self.host, self.port, key)? {
            HostKeyStatus::Known => Ok(true),
            HostKeyStatus::Changed { line } => Err(anyhow!(
                "REMOTE HOST IDENTIFICATION HAS CHANGED for {}:{} (see {} line {}) \u{2014} refusing to connect",
                self.host,
                self.port,
                known_hosts.display(),
                line
            )),
            HostKeyStatus::Unknown => {
                if !self.ask_to_trust(key).await {
                    return Err(HostKeyDeclined.into());
                }
                if let Err(err) = trust_host_key(known_hosts, &self.host, self.port, key) {
                    tracing::warn!("could not save the host key for {}:{}: {err:#}", self.host, self.port);
                }
                Ok(true)
            }
        }
    }

    async fn ask_to_trust(&self, key: &PublicKey) -> bool {
        let (reply, answer) = oneshot::channel();
        let question = HostKeyQuestion {
            key_type: key.algorithm().to_string(),
            fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
            reply,
        };
        if self.questions.send(question).await.is_err() {
            return false;
        }
        answer.await.unwrap_or(false)
    }
}

pub struct PorthmosHandler {
    host_key: HostKeyCheck,
}

impl client::Handler for PorthmosHandler {
    type Error = anyhow::Error;

    async fn check_server_key(&mut self, server_public_key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = server_public_key else {
            return Err(anyhow!(
                "the host key for {} is a certificate, which Porthmos does not yet support",
                self.host_key.host
            ));
        };
        self.host_key.verify(key).await
    }
}

pub fn software_version(seen: &[u8]) -> Option<String> {
    let end = seen.iter().position(|byte| *byte == b'\n')?;
    let line = String::from_utf8_lossy(&seen[..end]);
    let rest = line.trim_end_matches('\r').strip_prefix("SSH-")?;
    let (_protocol, software) = rest.split_once('-')?;
    clean_server_text(software, SERVER_INFO_CELLS)
}

#[derive(Clone, Default)]
pub struct ServerId(Arc<Mutex<Vec<u8>>>);

impl ServerId {
    fn bytes(&self) -> std::sync::MutexGuard<'_, Vec<u8>> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn record(&self, bytes: &[u8]) {
        let mut seen = self.bytes();
        if seen.contains(&b'\n') || seen.len() >= SNIFF_LIMIT {
            return;
        }
        let room = SNIFF_LIMIT - seen.len();
        seen.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    pub fn version(&self) -> Option<String> {
        software_version(&self.bytes())
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.bytes().len()
    }
}

pub(crate) struct Sniffing<R> {
    inner: R,
    seen: ServerId,
}

impl<R> Sniffing<R> {
    pub(crate) fn new(inner: R, seen: ServerId) -> Self {
        Self { inner, seen }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Sniffing<R> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &polled {
            self.seen.record(&buf.filled()[before..]);
        }
        polled
    }
}

impl<R: AsyncWrite + Unpin> AsyncWrite for Sniffing<R> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, data: &[u8]) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

pub async fn connect(host_key: HostKeyCheck) -> Result<(Handle<PorthmosHandler>, ServerId)> {
    let config = Arc::new(client::Config::default());
    let address = (host_key.host.clone(), host_key.port);
    let stream = tokio::net::TcpStream::connect(address).await.map_err(russh::Error::IO)?;
    if config.nodelay
        && let Err(err) = stream.set_nodelay(true)
    {
        tracing::warn!("set_nodelay() failed: {err:?}");
    }
    let seen = ServerId::default();
    let handle =
        client::connect_stream(config, Sniffing::new(stream, seen.clone()), PorthmosHandler { host_key }).await?;
    Ok((handle, seen))
}

pub async fn authenticate_non_interactive(
    handle: &mut Handle<PorthmosHandler>, username: &str, identity_file: Option<&Path>, password: Option<&str>,
    use_agent: bool,
) -> Result<bool> {
    if use_agent && authenticate_with_agent(handle, username).await? {
        return Ok(true);
    }

    if let Some(identity_file) = identity_file
        && authenticate_with_key_file(handle, username, identity_file).await?
    {
        return Ok(true);
    }

    if let Some(password) = password
        && authenticate_password(handle, username, password).await?
    {
        return Ok(true);
    }

    Ok(false)
}

async fn authenticate_with_agent(handle: &mut Handle<PorthmosHandler>, username: &str) -> Result<bool> {
    let Ok(mut agent) = AgentClient::connect_env().await else {
        return Ok(false);
    };

    let Ok(identities) = agent.request_identities().await else {
        return Ok(false);
    };

    for identity in identities {
        let public_key = identity.public_key().into_owned();
        let mut signer = AgentSigner(&mut agent);

        let result = handle.authenticate_publickey_with(username, public_key, None, &mut signer).await;

        if let Ok(AuthResult::Success) = result {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn authenticate_with_key_file(
    handle: &mut Handle<PorthmosHandler>, username: &str, identity_file: &Path,
) -> Result<bool> {
    let Ok(private_key) = PrivateKey::read_openssh_file(identity_file) else {
        return Ok(false);
    };

    if private_key.is_encrypted() {
        return Ok(false);
    }

    let key = PrivateKeyWithHashAlg::new(Arc::new(private_key), None);

    match handle.authenticate_publickey(username, key).await? {
        AuthResult::Success => Ok(true),
        AuthResult::Failure { .. } => Ok(false),
    }
}

pub async fn authenticate_password(
    handle: &mut Handle<PorthmosHandler>, username: &str, password: &str,
) -> Result<bool> {
    match handle.authenticate_password(username, password).await? {
        AuthResult::Success => Ok(true),
        AuthResult::Failure { .. } => Ok(false),
    }
}

struct AgentSigner<'a>(&'a mut AgentClient<tokio::net::UnixStream>);

impl Signer for AgentSigner<'_> {
    type Error = anyhow::Error;

    async fn auth_sign(
        &mut self, key: &AgentIdentity, hash_alg: Option<HashAlg>, to_sign: Vec<u8>,
    ) -> Result<Vec<u8>, Self::Error> {
        let public_key = key.public_key().into_owned();
        let signature = self.0.sign_request_signature(&public_key, hash_alg, &to_sign).await?;
        Ok(signature.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests;

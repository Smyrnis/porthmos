use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsString,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio::task::JoinHandle;

use porthmos_vfs::{Answer, Prompter, Question, Target, async_trait};
use russh::{
    Channel, ChannelId, ChannelMsg, ChannelOpenFailure,
    server::{self, Auth, Msg, Server as _, Session as ServerSession},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

use crate::{ConnectOptions, Session, connect};

pub const USER: &str = "u";
pub const PASSWORD: &str = "pw";

const HOST_KEY: &str = include_str!("host_key");

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub hide_scp: bool,
    pub refuse_exec: bool,
    pub refuse_shell: bool,
    pub max_sessions: Option<usize>,
    pub forwarding: bool,
    pub replace: &'static [(&'static str, &'static str)],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyRequest {
    pub term: String,
    pub columns: u32,
    pub rows: u32,
}

struct RemoteListeners {
    tasks: Mutex<HashMap<u32, JoinHandle<()>>>,
    live: Arc<AtomicUsize>,
}

impl RemoteListeners {
    fn new(live: Arc<AtomicUsize>) -> Arc<Self> {
        Arc::new(Self { tasks: Mutex::new(HashMap::new()), live })
    }
}

impl Drop for RemoteListeners {
    fn drop(&mut self) {
        let tasks = self.tasks.get_mut().unwrap_or_else(|poisoned| poisoned.into_inner());
        self.live.fetch_sub(tasks.len(), Ordering::SeqCst);
        for task in tasks.values() {
            task.abort();
        }
    }
}

#[derive(Clone)]
struct Handler {
    remote: Arc<RemoteListeners>,
    sessions: Arc<Mutex<Vec<server::Handle>>>,
    announced: bool,
    root: PathBuf,
    path: OsString,
    options: Options,
    channels: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
    open: Arc<Mutex<HashSet<ChannelId>>>,
    running: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    started: Arc<AtomicUsize>,
    forwarded: Arc<AtomicUsize>,
    pty: Arc<Mutex<Option<PtyRequest>>>,
    window: Arc<Mutex<Option<(u32, u32)>>>,
    environment: Arc<Mutex<Vec<(String, String)>>>,
}

impl server::Server for Handler {
    type Handler = Self;

    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Self {
            channels: Arc::default(),
            open: Arc::default(),
            remote: RemoteListeners::new(self.remote.live.clone()),
            announced: false,
            ..self.clone()
        }
    }
}

async fn relay(mut child: tokio::process::Child, channel: Channel<Msg>) -> Option<u32> {
    let (mut read, write) = channel.split();
    let mut stdin = child.stdin.take()?;
    let mut stdout = child.stdout.take()?;
    let mut stderr = child.stderr.take()?;
    let feed = tokio::spawn(async move {
        while let Some(message) = read.wait().await {
            match message {
                ChannelMsg::Data { data } => {
                    let written = stdin.write_all(&data).await;
                    if written.is_err() {
                        break;
                    }
                }
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        }
    });
    let mut broken = false;
    let out = async {
        let mut buffer = vec![0u8; 32 * 1024];
        while let Ok(count) = stdout.read(&mut buffer).await {
            if count == 0 {
                break;
            }
            if write.data(&buffer[..count]).await.is_err() {
                broken = true;
                break;
            }
        }
    };
    let err = async {
        let mut buffer = vec![0u8; 4 * 1024];
        while let Ok(count) = stderr.read(&mut buffer).await {
            if count == 0 || write.extended_data(1, &buffer[..count]).await.is_err() {
                break;
            }
        }
    };
    tokio::join!(out, err);
    if broken {
        let _ = child.start_kill();
    }
    let status = child.wait().await.ok()?.code().unwrap_or(1) as u32;
    feed.abort();
    let _ = write.exit_status(status).await;
    let _ = write.eof().await;
    let _ = write.close().await;
    Some(status)
}

impl server::Handler for Handler {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if user == USER && password == PASSWORD { Auth::Accept } else { Auth::reject() })
    }

    async fn channel_open_session(
        &mut self, channel: Channel<Msg>, reply: server::ChannelOpenHandle, session: &mut ServerSession,
    ) -> Result<(), Self::Error> {
        if !self.announced {
            self.announced = true;
            self.sessions.lock().unwrap().push(session.handle());
        }
        if self.options.max_sessions.is_some_and(|max| self.open.lock().unwrap().len() >= max) {
            reply.reject(ChannelOpenFailure::ResourceShortage).await;
            return Ok(());
        }
        self.open.lock().unwrap().insert(channel.id());
        self.channels.lock().unwrap().insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self, channel: Channel<Msg>, host_to_connect: &str, port_to_connect: u32, _: &str, _: u32,
        reply: server::ChannelOpenHandle, _: &mut ServerSession,
    ) -> Result<(), Self::Error> {
        if !self.options.forwarding {
            reply.reject(ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        }
        let Ok(mut upstream) = tokio::net::TcpStream::connect((host_to_connect, port_to_connect as u16)).await else {
            reply.reject(ChannelOpenFailure::ConnectFailed).await;
            return Ok(());
        };
        reply.accept().await;
        self.forwarded.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut upstream).await;
        });
        Ok(())
    }

    async fn tcpip_forward(
        &mut self, address: &str, port: &mut u32, session: &mut ServerSession,
    ) -> Result<bool, Self::Error> {
        if !self.options.forwarding {
            return Ok(false);
        }
        let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", *port as u16)).await else {
            return Ok(false);
        };
        let Ok(bound) = listener.local_addr() else {
            return Ok(false);
        };
        let bound_port = u32::from(bound.port());
        *port = bound_port;
        let handle = session.handle();
        if !self.announced {
            self.announced = true;
            self.sessions.lock().unwrap().push(handle.clone());
        }
        let address = address.to_string();
        let task = tokio::spawn(async move {
            while let Ok((mut stream, peer)) = listener.accept().await {
                let opened = handle
                    .channel_open_forwarded_tcpip(
                        address.clone(),
                        bound_port,
                        peer.ip().to_string(),
                        u32::from(peer.port()),
                    )
                    .await;
                let Ok(channel) = opened else { continue };
                tokio::spawn(async move {
                    let mut channel_stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut channel_stream, &mut stream).await;
                });
            }
        });
        let mut tasks = self.remote.tasks.lock().unwrap();
        if tasks.insert(bound_port, task).is_none() {
            self.remote.live.fetch_add(1, Ordering::SeqCst);
        }
        Ok(true)
    }

    async fn cancel_tcpip_forward(
        &mut self, _address: &str, port: u32, _: &mut ServerSession,
    ) -> Result<bool, Self::Error> {
        let task = self.remote.tasks.lock().unwrap().remove(&port);
        let Some(task) = task else { return Ok(false) };
        self.remote.live.fetch_sub(1, Ordering::SeqCst);
        task.abort();
        let _ = task.await;
        Ok(true)
    }

    async fn channel_close(&mut self, id: ChannelId, _: &mut ServerSession) -> Result<(), Self::Error> {
        self.channels.lock().unwrap().remove(&id);
        self.open.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn exec_request(
        &mut self, id: ChannelId, data: &[u8], session: &mut ServerSession,
    ) -> Result<(), Self::Error> {
        let Some(channel) = self.channels.lock().unwrap().remove(&id) else {
            return session.channel_failure(id);
        };
        if self.options.refuse_exec {
            return self.refuse(id, channel, session).await;
        }
        session.channel_success(id)?;
        let requested = String::from_utf8_lossy(data).into_owned();
        let command = self
            .options
            .replace
            .iter()
            .find(|(prefix, _)| requested.starts_with(prefix))
            .map_or(requested.clone(), |(_, replacement)| replacement.to_string());
        self.run(id, channel, command)
    }

    async fn pty_request(
        &mut self, id: ChannelId, term: &str, columns: u32, rows: u32, _: u32, _: u32, _: &[(russh::Pty, u32)],
        session: &mut ServerSession,
    ) -> Result<(), Self::Error> {
        *self.pty.lock().unwrap() = Some(PtyRequest { term: term.to_string(), columns, rows });
        session.channel_success(id)
    }

    async fn shell_request(&mut self, id: ChannelId, session: &mut ServerSession) -> Result<(), Self::Error> {
        let Some(channel) = self.channels.lock().unwrap().remove(&id) else {
            return session.channel_failure(id);
        };
        if self.options.refuse_shell {
            return self.refuse(id, channel, session).await;
        }
        session.channel_success(id)?;
        self.run(id, channel, "sh".to_string())
    }

    async fn env_request(
        &mut self, _: ChannelId, name: &str, value: &str, _: &mut ServerSession,
    ) -> Result<(), Self::Error> {
        self.environment.lock().unwrap().push((name.to_string(), value.to_string()));
        Ok(())
    }

    async fn window_change_request(
        &mut self, _: ChannelId, columns: u32, rows: u32, _: u32, _: u32, _: &mut ServerSession,
    ) -> Result<(), Self::Error> {
        *self.window.lock().unwrap() = Some((columns, rows));
        Ok(())
    }
}

impl Handler {
    async fn refuse(
        &mut self, id: ChannelId, channel: Channel<Msg>, session: &mut ServerSession,
    ) -> Result<(), russh::Error> {
        self.open.lock().unwrap().remove(&id);
        session.channel_failure(id)?;
        let _ = channel.close().await;
        Ok(())
    }

    fn run(&mut self, id: ChannelId, channel: Channel<Msg>, command: String) -> Result<(), russh::Error> {
        let child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.root)
            .env("PATH", &self.path)
            .env("HOME", &self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(russh::Error::IO)?;
        let (running, peak, open) = (self.running.clone(), self.peak.clone(), self.open.clone());
        self.started.fetch_add(1, Ordering::SeqCst);
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(now, Ordering::SeqCst);
        tokio::spawn(async move {
            relay(child, channel).await;
            running.fetch_sub(1, Ordering::SeqCst);
            open.lock().unwrap().remove(&id);
        });
        Ok(())
    }
}

#[cfg(windows)]
fn search_path(_hide_scp: bool) -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

#[cfg(unix)]
fn search_path(hide_scp: bool) -> OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if !hide_scp {
        return path;
    }
    let tools = tempfile::tempdir().expect("a temporary directory").keep();
    for tool in ["sh", "ls", "cat", "tail", "mkdir", "rm", "mv", "printf", "sleep", "mkfifo", "head", "env"] {
        if let Some(found) = std::env::split_paths(&path).map(|dir| dir.join(tool)).find(|candidate| candidate.exists())
        {
            let _ = std::os::unix::fs::symlink(found, tools.join(tool));
        }
    }
    std::env::join_paths([tools]).unwrap_or_default()
}

pub struct SshServer {
    remote_live: Arc<AtomicUsize>,
    sessions: Arc<Mutex<Vec<server::Handle>>>,
    pub port: u16,
    pub root: tempfile::TempDir,
    known_hosts: tempfile::TempDir,
    peak: Arc<AtomicUsize>,
    started: Arc<AtomicUsize>,
    forwarded: Arc<AtomicUsize>,
    running: Arc<AtomicUsize>,
    pty: Arc<Mutex<Option<PtyRequest>>>,
    window: Arc<Mutex<Option<(u32, u32)>>>,
    environment: Arc<Mutex<Vec<(String, String)>>>,
}

impl SshServer {
    pub async fn start(options: Options) -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let config = Arc::new(server::Config {
            keys: vec![russh::keys::PrivateKey::from_openssh(HOST_KEY).expect("the test host key parses")],
            auth_rejection_time: std::time::Duration::from_millis(1),
            auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)),
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("a free port");
        let port = listener.local_addr().expect("a bound address").port();
        let peak = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let forwarded = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicUsize::new(0));
        let pty = Arc::new(Mutex::new(None));
        let window = Arc::new(Mutex::new(None));
        let environment = Arc::new(Mutex::new(Vec::new()));
        let remote_live = Arc::new(AtomicUsize::new(0));
        let sessions = Arc::new(Mutex::new(Vec::new()));
        let mut handler = Handler {
            remote: RemoteListeners::new(remote_live.clone()),
            sessions: sessions.clone(),
            announced: false,
            root: root.path().to_path_buf(),
            path: search_path(options.hide_scp),
            options,
            channels: Arc::default(),
            open: Arc::default(),
            running: running.clone(),
            peak: peak.clone(),
            started: started.clone(),
            forwarded: forwarded.clone(),
            pty: pty.clone(),
            window: window.clone(),
            environment: environment.clone(),
        };
        tokio::spawn(async move {
            let _ = handler.run_on_socket(config, &listener).await;
        });
        Self {
            remote_live,
            sessions,
            port,
            root,
            known_hosts: tempfile::tempdir().expect("a temporary directory"),
            peak,
            started,
            forwarded,
            running,
            pty,
            window,
            environment,
        }
    }

    pub fn known_hosts(&self) -> PathBuf {
        self.known_hosts.path().join("known_hosts")
    }

    pub fn connect_options(&self) -> ConnectOptions {
        ConnectOptions { known_hosts: Some(self.known_hosts()), use_agent: false }
    }

    pub fn target(&self) -> Target {
        Target {
            name: "box".into(),
            host: "127.0.0.1".into(),
            port: self.port,
            username: USER.into(),
            password: Some(PASSWORD.into()),
            options: Default::default(),
        }
    }

    pub async fn session(&self) -> Session {
        connect(&self.target(), &mut answers(vec![Some(Answer::Confirmed)]), &self.connect_options())
            .await
            .expect("the test server accepts the password")
    }

    pub fn peak_commands(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    pub fn forwarded_connections(&self) -> usize {
        self.forwarded.load(Ordering::SeqCst)
    }

    pub fn remote_listeners(&self) -> usize {
        self.remote_live.load(Ordering::SeqCst)
    }

    pub async fn open_forwarded_channel(&self, address: &str, port: u16) -> bool {
        let handle = self.sessions.lock().unwrap().last().cloned();
        let Some(handle) = handle else { return false };
        match handle.channel_open_forwarded_tcpip(address, u32::from(port), "127.0.0.1", 40000).await {
            Ok(channel) => {
                tokio::spawn(async move {
                    let _stream = channel.into_stream();
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                });
                true
            }
            Err(_) => false,
        }
    }

    pub async fn disconnect_clients(&self) {
        let handles: Vec<_> = self.sessions.lock().unwrap().clone();
        for handle in handles {
            let _ = handle.disconnect(russh::Disconnect::ByApplication, String::new(), String::new()).await;
        }
    }

    pub fn running_commands(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }

    pub fn started_commands(&self) -> usize {
        self.started.load(Ordering::SeqCst)
    }

    pub fn last_pty_request(&self) -> Option<PtyRequest> {
        self.pty.lock().unwrap().clone()
    }

    pub fn received_environment(&self) -> Vec<(String, String)> {
        self.environment.lock().unwrap().clone()
    }

    pub fn last_window_size(&self) -> Option<(u32, u32)> {
        *self.window.lock().unwrap()
    }
}

pub struct Answers {
    pub answers: VecDeque<Option<Answer>>,
    pub asked: Vec<Question>,
}

pub fn answers(answers: Vec<Option<Answer>>) -> Answers {
    Answers { answers: answers.into(), asked: Vec::new() }
}

#[async_trait]
impl Prompter for Answers {
    async fn ask(&mut self, question: Question) -> Option<Answer> {
        self.asked.push(question);
        self.answers.pop_front().flatten()
    }
}

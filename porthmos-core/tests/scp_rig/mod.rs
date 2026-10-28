#![allow(dead_code)]

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use porthmos_core::{
    Answer, Command, Core, CoreHandle, Entry, Event, Location, Paths, Protocol, Question, Severity,
    config::{Settings, settings::TransferSettings},
    history::HistoryEntry,
    sync::{SyncOptions, SyncPlan},
    transfer::{
        conflicts::{ConflictInfo, ConflictPolicy, Resolution},
        rows::RowState,
    },
};
use porthmos_lfs::from_os_path;
#[cfg(unix)]
use porthmos_scp::Scp;
#[cfg(unix)]
use porthmos_ssh::testing::{Options, PASSWORD, SshServer, USER};
use tokio::sync::mpsc::UnboundedReceiver;

pub struct Rig {
    pub core: CoreHandle,
    events: UnboundedReceiver<Event>,
    pub remote_root: PathBuf,
    local: tempfile::TempDir,
    pub session: u64,
    pub max_active: usize,
    pub notices: Vec<String>,
    _config: tempfile::TempDir,
    _remote: RemoteHold,
}

struct Remote {
    profile: String,
    root: PathBuf,
    password: Option<&'static str>,
    protocol: Option<Arc<dyn Protocol>>,
    hold: RemoteHold,
}

#[cfg(unix)]
type RemoteHold = SshServer;
#[cfg(windows)]
type RemoteHold = tempfile::TempDir;

pub fn data(size: usize, seed: u8) -> Vec<u8> {
    (0..size).map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

#[cfg(unix)]
async fn remote_side() -> Remote {
    let server = SshServer::start(Options::default()).await;
    let profile = format!(
        "[connections.box]\nprotocol = \"scp\"\nhost = \"127.0.0.1\"\nport = {}\nusername = \"{USER}\"\n",
        server.port
    );
    let scp = Scp::default().with_connect_options(server.connect_options());
    Remote {
        profile,
        root: server.root.path().to_path_buf(),
        password: Some(PASSWORD),
        protocol: Some(Arc::new(scp)),
        hold: server,
    }
}

#[cfg(windows)]
async fn remote_side() -> Remote {
    let variable = |name: &str| {
        std::env::var(name)
            .unwrap_or_else(|_| panic!("set PORTHMOS_TEST_SSHD_* or run without the sshd-tests feature ({name})"))
    };
    let port = variable("PORTHMOS_TEST_SSHD_PORT");
    let user = variable("PORTHMOS_TEST_SSHD_USER");
    let key = variable("PORTHMOS_TEST_SSHD_KEY").replace('\\', "/");
    let folder = tempfile::tempdir().unwrap();
    let profile = format!(
        "[connections.box]\nprotocol = \"sftp\"\nhost = \"127.0.0.1\"\nport = {port}\nusername = \"{user}\"\nidentity_file = \"{key}\"\n"
    );
    Remote { profile, root: folder.path().to_path_buf(), password: None, protocol: None, hold: folder }
}

pub async fn rig(max_parallel: usize, policy: ConflictPolicy) -> Rig {
    let remote = remote_side().await;
    let config = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(config.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.connections_file(), &remote.profile).unwrap();
    let settings =
        Settings { transfers: TransferSettings { max_parallel, on_conflict: policy }, ..Settings::default() };
    let builder =
        Core::builder().without_keyring().paths(paths).settings(settings).local_home(local.path().to_path_buf());
    let builder = match remote.protocol {
        Some(protocol) => builder.protocol(protocol),
        None => builder,
    };
    let (core, events) = builder.start().unwrap();
    let mut rig = Rig {
        core,
        events,
        remote_root: remote.root,
        local,
        session: 0,
        max_active: 0,
        notices: Vec::new(),
        _config: config,
        _remote: remote.hold,
    };
    rig.core.send(Command::Connect { profile: "box".into() });
    loop {
        match rig.event().await {
            Event::Question { request_id, question: Question::TrustHostKey { .. } } => {
                rig.core.send(Command::Answer { request_id, answer: Some(Answer::Confirmed), save: false })
            }
            Event::Question { request_id, question: Question::Password { .. } } => rig.core.send(Command::Answer {
                request_id,
                answer: Some(Answer::Password(remote.password.expect("the server asked for a password").into())),
                save: false,
            }),
            Event::Connected { session, .. } => {
                rig.session = session;
                return rig;
            }
            Event::ConnectFailed { message, .. } => panic!("connect failed: {message}"),
            _ => {}
        }
    }
}

impl Rig {
    pub async fn event(&mut self) -> Event {
        let event = tokio::time::timeout(Duration::from_secs(30), self.events.recv())
            .await
            .expect("an event within 30 seconds")
            .expect("the core is running");
        match &event {
            Event::TransfersChanged(snapshot) => self.max_active = self.max_active.max(snapshot.active.len()),
            Event::Notice { message, .. } => self.notices.push(message.clone()),
            _ => {}
        }
        event
    }

    pub fn state_paths(&self) -> Paths {
        Paths::in_dir(self._config.path())
    }

    pub async fn history(&mut self) -> Vec<HistoryEntry> {
        self.core.send(Command::ListHistory);
        loop {
            if let Event::History(entries) = self.event().await {
                return entries;
            }
        }
    }

    pub async fn open_for_edit(&mut self, name: &str) -> (u64, PathBuf) {
        self.core.send(Command::EditFile {
            location: Location::Session(self.session),
            path: from_os_path(&self.remote(name)),
        });
        loop {
            if let Event::EditReady { edit_id, file, .. } = self.event().await {
                return (edit_id, file);
            }
        }
    }

    pub async fn wait_for_notice(&mut self, containing: &str) {
        loop {
            if let Event::Notice { message, .. } = self.event().await
                && message.contains(containing)
            {
                return;
            }
        }
    }

    pub fn remote(&self, name: &str) -> PathBuf {
        self.remote_root.join(name)
    }

    pub fn local(&self, name: &str) -> PathBuf {
        self.local.path().join(name)
    }

    fn entry(path: &Path, is_dir: bool) -> Entry {
        let size = if is_dir { 0 } else { std::fs::metadata(path).unwrap().len() };
        Entry {
            modified: None,
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path: from_os_path(path),
            is_dir,
            size,
            permissions: None,
            ..Default::default()
        }
    }

    pub fn upload(&self, names: &[(&str, bool)]) {
        let entries = names.iter().map(|(name, is_dir)| Self::entry(&self.local(name), *is_dir)).collect();
        self.core.send(Command::Copy {
            from: Location::Local,
            entries,
            to: Location::Session(self.session),
            dest_dir: from_os_path(&self.remote_root),
        });
    }

    pub fn download(&self, names: &[(&str, bool)]) {
        let entries = names.iter().map(|(name, is_dir)| Self::entry(&self.remote(name), *is_dir)).collect();
        self.core.send(Command::Copy {
            from: Location::Session(self.session),
            entries,
            to: Location::Local,
            dest_dir: from_os_path(self.local.path()),
        });
    }

    pub fn move_up(&self, names: &[(&str, bool)]) {
        let entries = names.iter().map(|(name, is_dir)| Self::entry(&self.local(name), *is_dir)).collect();
        self.core.send(Command::Move {
            from: Location::Local,
            entries,
            to: Location::Session(self.session),
            dest_dir: from_os_path(&self.remote_root),
        });
    }

    pub fn move_down(&self, names: &[(&str, bool)]) {
        let entries = names.iter().map(|(name, is_dir)| Self::entry(&self.remote(name), *is_dir)).collect();
        self.core.send(Command::Move {
            from: Location::Session(self.session),
            entries,
            to: Location::Local,
            dest_dir: from_os_path(self.local.path()),
        });
    }

    pub async fn settle(&mut self, answers: impl Fn(&[ConflictInfo]) -> Option<Vec<Resolution>>) -> Vec<RowState> {
        let mut started = false;
        loop {
            match self.event().await {
                Event::ConflictsFound { batch_id, files } => {
                    self.core.send(Command::ResolveConflicts { batch_id, answers: answers(&files) });
                }
                Event::TransfersChanged(snapshot) => {
                    let busy = !snapshot.active.is_empty()
                        || snapshot.queued > 0
                        || !snapshot.scanning.is_empty()
                        || snapshot.awaiting_answers > 0;
                    started |= !snapshot.rows.is_empty();
                    if !busy && started {
                        return snapshot.rows.iter().map(|row| row.state).collect();
                    }
                }
                _ => {}
            }
        }
    }

    pub async fn settle_with_no_rows(
        &mut self, answers: impl Fn(&[ConflictInfo]) -> Option<Vec<Resolution>>, notice: &str,
    ) {
        loop {
            match self.event().await {
                Event::ConflictsFound { batch_id, files } => {
                    self.core.send(Command::ResolveConflicts { batch_id, answers: answers(&files) });
                }
                Event::Notice { message, .. } if message.contains(notice) => return,
                _ => {}
            }
        }
    }

    pub async fn scan_sync(&mut self, options: SyncOptions) -> Option<Arc<SyncPlan>> {
        self.core.send(Command::StartSync {
            session: self.session,
            local_dir: from_os_path(&self.local("tree")),
            remote_dir: from_os_path(&self.remote("tree")),
            options,
        });
        loop {
            match self.event().await {
                Event::SyncPlanReady(plan) => return Some(plan),
                Event::Notice { message, .. } if message == "Folders are in sync" => return None,
                Event::Notice { severity: Severity::Error, message } => panic!("{message}"),
                _ => {}
            }
        }
    }

    pub async fn run_sync(&mut self, plan: &SyncPlan) -> Vec<RowState> {
        let choices: Vec<_> = plan.items.iter().filter(|item| item.ticked).map(|item| (item.id, item.action)).collect();
        let nothing_ticked = choices.is_empty();
        self.core.send(Command::RunSync { sync_id: plan.sync_id, choices });
        if nothing_ticked {
            self.wait_for_notice("Nothing to sync").await;
            return Vec::new();
        }
        self.settle(|_| None).await
    }

    pub async fn drain(&mut self) {
        while tokio::time::timeout(Duration::from_millis(300), self.events.recv()).await.is_ok_and(|event| {
            if let Some(Event::Notice { message, .. }) = &event {
                self.notices.push(message.clone());
            }
            event.is_some()
        }) {}
    }
}

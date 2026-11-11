pub mod config;
mod connection_state;
pub mod connections_tree;
pub mod delete_prompt;
pub mod edit;
mod engine;
pub mod error;
mod health;
pub mod history;
pub mod last_used;
pub mod listing;
pub mod logging;
pub mod message;
pub mod names;
pub mod panel_memory;
pub mod permissions;
mod path_text;
pub mod paths;
pub mod persist;
pub mod preview;
pub mod profiles;
mod protocol_info;
pub mod reopen;
pub mod secrets;
pub mod size_text;
pub mod state;
pub mod sync;
pub mod tasks;
mod test_connection;
pub mod transfer;
pub mod update;

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
pub use config::{ReopenMode, SearchSettings, StartupSettings, UpdateMode, UpdateSettings};
pub use connection_state::ConnectionState;
pub use engine::{Command, Event, Location, RequestId, SessionId};
pub use error::{CONNECTION_CANCELLED, Severity, connect_failure_message, failure_report, user_message};
pub use message::Message;
pub use panel_memory::PanelKind;
pub use paths::{ConfigMigration, Paths};
pub use porthmos_lfs::{from_os_path, to_os_path};
pub use porthmos_vfs::{
    Answer, Choice, CommonField, ConnectionForm, DirItem, Entry, Environment, ErrorKind, FileKind, FileSystem,
    Metadata, OptionField, OptionKind, PortField, Protocol, ProtocolError, Question, SearchEvent, SearchQuery,
    ShellInvocation, Target, VfsPath, glob_match,
};
pub use protocol_info::ProtocolInfo;
pub use reopen::offer_summary;
pub use size_text::size_text;
pub use test_connection::{NotTrusted, StartFolder, TEST_LIMIT, TestOutcome, TestReport, TestStage};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
#[cfg(feature = "update")]
pub use update::{AppIdentity, Product};
pub use update::{ApplyOutcome, ExitLine, HeadlessUpdate, UpdateState, apply_staged_update, run_update};

use crate::{
    config::Settings,
    engine::{Engine, EngineParts},
};

#[cfg(feature = "sftp")]
fn sftp_protocol() -> Option<Arc<dyn Protocol>> {
    Some(Arc::new(porthmos_sftp::Sftp))
}

#[cfg(not(feature = "sftp"))]
fn sftp_protocol() -> Option<Arc<dyn Protocol>> {
    None
}

#[cfg(feature = "ftp")]
fn ftp_protocol(paths: &Paths) -> Option<Arc<dyn Protocol>> {
    Some(Arc::new(porthmos_ftp::Ftp::new(paths.known_certificates_file())))
}

#[cfg(not(feature = "ftp"))]
fn ftp_protocol(_paths: &Paths) -> Option<Arc<dyn Protocol>> {
    None
}

#[cfg(feature = "webdav")]
fn webdav_protocol(paths: &Paths) -> Option<Arc<dyn Protocol>> {
    Some(Arc::new(porthmos_webdav::WebDav::new(paths.known_certificates_file())))
}

#[cfg(not(feature = "webdav"))]
fn webdav_protocol(_paths: &Paths) -> Option<Arc<dyn Protocol>> {
    None
}

#[cfg(feature = "s3")]
fn s3_protocol(paths: &Paths) -> Option<Arc<dyn Protocol>> {
    Some(Arc::new(porthmos_s3::S3::new(paths.known_certificates_file())))
}

#[cfg(not(feature = "s3"))]
fn s3_protocol(_paths: &Paths) -> Option<Arc<dyn Protocol>> {
    None
}

#[cfg(feature = "scp")]
fn scp_protocol() -> Option<Arc<dyn Protocol>> {
    Some(Arc::new(porthmos_scp::Scp::default()))
}

#[cfg(not(feature = "scp"))]
fn scp_protocol() -> Option<Arc<dyn Protocol>> {
    None
}

pub fn builtin_protocols(paths: &Paths) -> Vec<Arc<dyn Protocol>> {
    [sftp_protocol(), ftp_protocol(paths), webdav_protocol(paths), s3_protocol(paths), scp_protocol()]
        .into_iter()
        .flatten()
        .collect()
}

#[derive(Clone)]
pub struct CoreHandle {
    commands: UnboundedSender<Command>,
    protocols: Arc<[ProtocolInfo]>,
    finished: tokio::sync::watch::Receiver<bool>,
    local_folder: Option<VfsPath>,
    open_groups: Vec<String>,
    panels: Panels,
}

#[derive(Debug, Clone, Default)]
struct Panels {
    local: config::settings::PanelSettings,
    remote: config::settings::PanelSettings,
}

impl CoreHandle {
    pub fn with_panel_settings(mut self, kind: PanelKind, settings: config::settings::PanelSettings) -> Self {
        match kind {
            PanelKind::Local => self.panels.local = settings,
            PanelKind::Remote => self.panels.remote = settings,
        }
        self
    }

    pub fn panel_settings(&self, kind: PanelKind) -> config::settings::PanelSettings {
        match kind {
            PanelKind::Local => self.panels.local,
            PanelKind::Remote => self.panels.remote,
        }
    }

    pub fn with_remembered_open_groups(mut self, groups: Vec<String>) -> Self {
        self.open_groups = groups;
        self
    }

    pub fn remembered_local_folder(&self) -> Option<VfsPath> {
        self.local_folder.clone()
    }

    pub fn remembered_open_groups(&self) -> &[String] {
        &self.open_groups
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    pub fn protocols(&self) -> &[ProtocolInfo] {
        &self.protocols
    }

    pub fn detached() -> (CoreHandle, UnboundedReceiver<Command>) {
        Self::detached_with(Vec::new())
    }

    pub fn detached_with(protocols: Vec<ProtocolInfo>) -> (CoreHandle, UnboundedReceiver<Command>) {
        let (commands, receiver) = unbounded_channel();
        let (_sender, finished) = tokio::sync::watch::channel(false);
        (
            CoreHandle {
                commands,
                protocols: protocols.into(),
                finished,
                local_folder: None,
                open_groups: Vec::new(),
                panels: Panels::default(),
            },
            receiver,
        )
    }

    pub async fn shutdown(&self) {
        self.send(Command::Shutdown);
        let mut finished = self.finished.clone();
        let _ = finished.wait_for(|done| *done).await;
    }
}

#[cfg(windows)]
pub fn disable_core_dumps() {}

#[cfg(unix)]
pub fn disable_core_dumps() {
    let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    unsafe {
        libc::setrlimit(libc::RLIMIT_CORE, &limit);
    }
}

pub struct Core;

impl Core {
    pub fn builder() -> CoreBuilder {
        CoreBuilder::default()
    }
}

#[cfg(feature = "update")]
fn choose_updater(
    injected: Option<Arc<dyn update::Updater>>, release_updates: bool, mode: config::UpdateMode, paths: &Paths,
    identity: &update::AppIdentity,
) -> (Option<update::SharedUpdater>, Option<String>) {
    if let Some(updater) = injected {
        return (Some(updater), None);
    }
    if !release_updates || mode == config::UpdateMode::Off {
        return (None, None);
    }
    match update::ReleaseUpdater::new(identity, paths.update_dir()) {
        Ok(updater) => (Some(Arc::new(updater)), None),
        Err(err) => (None, Some(format!("Couldn't prepare update checks: {err:#}"))),
    }
}

#[derive(Default)]
pub struct CoreBuilder {
    paths: Option<Paths>,
    env: Environment,
    settings: Settings,
    protocols: Option<Vec<Arc<dyn Protocol>>>,
    local_home: Option<PathBuf>,
    secrets: Option<secrets::Secrets>,
    #[cfg(feature = "update")]
    updater: Option<Arc<dyn update::Updater>>,
    #[cfg(feature = "update")]
    release_updates: bool,
    #[cfg(feature = "update")]
    identity: Option<update::AppIdentity>,
}

impl CoreBuilder {
    pub fn paths(mut self, paths: Paths) -> Self {
        self.paths = Some(paths);
        self
    }

    pub fn environment(mut self, env: Environment) -> Self {
        self.env = env;
        self
    }

    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    pub fn protocol(mut self, protocol: Arc<dyn Protocol>) -> Self {
        self.protocols.get_or_insert_with(Vec::new).push(protocol);
        self
    }

    pub fn local_home(mut self, home: PathBuf) -> Self {
        self.local_home = Some(home);
        self
    }

    pub fn secrets(mut self, secrets: secrets::Secrets) -> Self {
        self.secrets = Some(secrets);
        self
    }

    #[cfg(feature = "update")]
    pub fn release_updates(mut self, identity: update::AppIdentity) -> Self {
        self.release_updates = true;
        self.identity = Some(identity);
        self
    }

    #[cfg(feature = "update")]
    pub fn updater(mut self, updater: Arc<dyn update::Updater>) -> Self {
        self.updater = Some(updater);
        self
    }

    pub fn without_keyring(self) -> Self {
        self.secrets(secrets::Secrets::new(None))
    }

    pub fn start(self) -> Result<(CoreHandle, UnboundedReceiver<Event>)> {
        let paths = self.paths.context("the core needs its configuration paths")?;
        let configured_panel = self.settings.panel;
        #[cfg(feature = "update")]
        update::legacy::remove(&paths);
        let (history, history_warning) = history::History::load(&paths);
        let (last_used, last_used_warning) = last_used::LastUsed::load(&paths);
        #[cfg(feature = "update")]
        let identity =
            self.identity.unwrap_or_else(|| update::AppIdentity::new(update::Product::Tui, env!("CARGO_PKG_VERSION")));
        #[cfg(feature = "update")]
        let product_id = identity.product.id();
        #[cfg(not(feature = "update"))]
        let product_id = "tui";
        let (update_record, update_warning) = update::record::UpdateRecord::load(&paths, product_id);
        let (bookmarks, bookmark_warnings) = config::bookmarks::load(&paths)?;
        #[cfg(feature = "update")]
        let (updater, updater_warning) =
            choose_updater(self.updater, self.release_updates, self.settings.update.mode, &paths, &identity);
        #[cfg(not(feature = "update"))]
        let (updater, updater_warning): (Option<update::SharedUpdater>, Option<String>) = (None, None);
        let local_home = self.local_home.or_else(|| self.env.home.clone()).unwrap_or_else(|| PathBuf::from("/"));
        let protocols = self.protocols.unwrap_or_else(|| builtin_protocols(&paths));
        let infos: Arc<[ProtocolInfo]> =
            protocols.iter().map(|protocol| ProtocolInfo::from_protocol(protocol.as_ref())).collect();
        let parts = EngineParts {
            paths,
            protocols,
            local_fs: Arc::new(porthmos_lfs::LocalFs::new(local_home)),
            env: self.env,
            transfers: self.settings.transfers,
            bookmarks,
            secrets: self.secrets.unwrap_or_else(secrets::Secrets::native),
            history,
            last_used,
            edit: self.settings.edit,
            update_timing: update::UpdateTiming::for_settings(&self.settings.update),
            update: self.settings.update,
            reopen: self.settings.startup.reopen,
            update_record,
            updater,
            executable: std::env::current_exe().ok(),
            #[cfg(feature = "update")]
            identity,
            publish_interval: engine::PROGRESS_SNAPSHOT_INTERVAL,
            persist_interval: engine::PERSIST_INTERVAL,
        };

        let (events, event_receiver) = unbounded_channel();
        let (commands, command_receiver) = unbounded_channel();
        let (internal, internal_receiver) = unbounded_channel();
        let (finished_sender, finished) = tokio::sync::watch::channel(false);
        let mut engine = Engine::new(parts, events.clone(), internal);
        engine.finished = Some(finished_sender);
        if let Some(warning) = history_warning {
            engine.report(Severity::Warning, warning);
        }
        if let Some(warning) = last_used_warning {
            engine.report(Severity::Warning, warning);
        }
        for warning in update_warning.into_iter().chain(updater_warning) {
            engine.report(Severity::Warning, warning);
        }
        for warning in bookmark_warnings {
            engine.report(Severity::Warning, warning.0);
        }
        engine.publish_bookmarks();
        engine.publish_last_used();
        engine.load_ui_state();
        engine.offer_reopen();
        let local_folder =
            engine.ui_state.local_folder.as_deref().map(VfsPath::new).filter(|folder| to_os_path(folder).is_dir());
        let open_groups = engine.ui_state.open_groups.clone();
        let remembered = |memory: &Option<panel_memory::PanelMemory>| match memory {
            Some(memory) => memory.applied_to(&configured_panel),
            None => configured_panel,
        };
        let panels = Panels {
            local: remembered(&engine.ui_state.local_panel),
            remote: remembered(&engine.ui_state.remote_panel),
        };
        engine.start_keyring_probe();
        engine.start_updates();
        tokio::spawn(engine.run(command_receiver, internal_receiver));
        Ok((CoreHandle { commands, protocols: infos, finished, local_folder, open_groups, panels }, event_receiver))
    }
}

#[cfg(test)]
mod tests;

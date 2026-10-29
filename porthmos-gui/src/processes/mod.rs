use std::ffi::{OsStr, OsString};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use iced::Subscription;
use iced::futures::StreamExt;
use iced::futures::stream::unfold;
use porthmos_core::edit::EditorExit;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::editing::{CommandSpec, Platform, editor_exit};
use crate::terminal::{TerminalError, TerminalLaunch, launch};

#[derive(Clone)]
pub struct ProcessJob {
    pub token: u64,
    pub spec: CommandSpec,
    pub env: Vec<(OsString, OsString)>,
    pub cleanup: Vec<PathBuf>,
}

impl ProcessJob {
    pub fn to_command(&self) -> std::process::Command {
        let mut command = self.spec.to_command();
        command.envs(self.env.iter().map(|(key, value)| (key, value)));
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        command
    }
}

impl fmt::Debug for ProcessJob {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let env: Vec<(&OsString, &str)> = self.env.iter().map(|(key, _)| (key, "<redacted>")).collect();
        formatter
            .debug_struct("ProcessJob")
            .field("token", &self.token)
            .field("spec", &self.spec)
            .field("env", &env)
            .field("cleanup", &self.cleanup)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessEvent {
    Exited { token: u64, exit: EditorExit },
}

pub trait Spawner: Send + Sync {
    fn spawn(&self, job: ProcessJob, events: UnboundedSender<ProcessEvent>);
}

pub struct SystemSpawner;

impl Spawner for SystemSpawner {
    fn spawn(&self, job: ProcessJob, events: UnboundedSender<ProcessEvent>) {
        std::thread::spawn(move || {
            let exit = editor_exit(job.to_command().status());
            if exit != EditorExit::Success {
                for file in &job.cleanup {
                    let _ = std::fs::remove_file(file);
                }
            }
            let _ = events.send(ProcessEvent::Exited { token: job.token, exit });
        });
    }
}

#[derive(Clone)]
pub struct ProcessSource {
    receiver: Arc<Mutex<Option<UnboundedReceiver<ProcessEvent>>>>,
}

impl ProcessSource {
    fn take(&self) -> Option<UnboundedReceiver<ProcessEvent>> {
        self.receiver.lock().ok()?.take()
    }

    pub fn subscription(&self) -> Subscription<ProcessEvent> {
        Subscription::run_with(self.clone(), |source| match source.take() {
            Some(receiver) => {
                unfold(receiver, |mut receiver| async move { receiver.recv().await.map(|event| (event, receiver)) })
                    .boxed()
            }
            None => iced::futures::stream::empty().boxed(),
        })
    }
}

impl Hash for ProcessSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "porthmos-process-events".hash(state);
    }
}

pub fn channel() -> (UnboundedSender<ProcessEvent>, ProcessSource) {
    let (sender, receiver) = unbounded_channel();
    (sender, ProcessSource { receiver: Arc::new(Mutex::new(Some(receiver))) })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

pub fn resolve_in_path(name: &str, search: &OsStr) -> Option<String> {
    std::env::split_paths(search)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable_file(candidate))
        .and_then(|found| std::fs::canonicalize(found).ok())
        .and_then(|real| real.file_name().map(|file| file.to_string_lossy().to_string()))
}

pub type Lookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub struct Launcher {
    spawner: Box<dyn Spawner>,
    events: UnboundedSender<ProcessEvent>,
    platform: Platform,
    lookup: Lookup,
    terminal_variable: Option<String>,
    state_dir: PathBuf,
}

impl Launcher {
    pub fn system(events: UnboundedSender<ProcessEvent>, state_dir: PathBuf) -> Self {
        let lookup: Lookup = Box::new(|name| resolve_in_path(name, &std::env::var_os("PATH").unwrap_or_default()));
        Self {
            spawner: Box::new(SystemSpawner),
            events,
            platform: Platform::current(),
            lookup,
            terminal_variable: std::env::var("TERMINAL").ok().filter(|value| !value.trim().is_empty()),
            state_dir,
        }
    }

    #[cfg(test)]
    pub fn custom(
        spawner: Box<dyn Spawner>, events: UnboundedSender<ProcessEvent>, platform: Platform, lookup: Lookup,
        terminal_variable: Option<String>, state_dir: PathBuf,
    ) -> Self {
        Self { spawner, events, platform, lookup, terminal_variable, state_dir }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn set_state_dir(&mut self, state_dir: PathBuf) {
        self.state_dir = state_dir;
    }

    pub fn spawn(&self, job: ProcessJob) {
        self.spawner.spawn(job, self.events.clone());
    }

    pub fn terminal(
        &self, inner: CommandSpec, env: Vec<(OsString, OsString)>, wait: bool,
    ) -> Result<TerminalLaunch, TerminalError> {
        launch(inner, env, wait, self.platform, &*self.lookup, self.terminal_variable.as_deref(), &self.state_dir)
    }
}

#[cfg(test)]
mod tests;

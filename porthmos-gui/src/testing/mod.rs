use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use porthmos_core::{Command, CoreHandle, Event, ProtocolInfo, VfsPath};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::app::{App, Message};
use crate::editing::Platform;
use crate::native::{Attached, EventLog, Native, NativeError, NativeEvent};
use crate::processes::{Launcher, ProcessEvent, ProcessJob, Spawner};
use crate::runtime::EventSource;
use crate::shortcuts::Shortcuts;
use crate::theme::{Palette, ThemeSettings};

pub struct Harness {
    pub app: App,
    commands: UnboundedReceiver<Command>,
    start: VfsPath,
    scratch: tempfile::TempDir,
    launched: Arc<Mutex<Vec<ProcessJob>>>,
    installed: Arc<Mutex<Vec<String>>>,
}

struct FakeSpawner(Arc<Mutex<Vec<ProcessJob>>>);

impl Spawner for FakeSpawner {
    fn spawn(&self, job: ProcessJob, _events: tokio::sync::mpsc::UnboundedSender<ProcessEvent>) {
        self.0.lock().unwrap().push(job);
    }
}

impl Harness {
    pub fn new() -> Harness {
        Harness::with_protocols(Vec::new())
    }

    pub fn with_builtin_protocols() -> Harness {
        Harness::with_protocols(builtin_infos())
    }

    pub fn with_protocols(protocols: Vec<ProtocolInfo>) -> Harness {
        let (core, commands) = CoreHandle::detached_with(protocols);
        let (_sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let start = VfsPath::new("/home/tester");
        let palette = Palette::new(&ThemeSettings::default());
        let mut app = App::new(core, EventSource::new(receiver), start.clone(), palette, Shortcuts::default());
        let scratch = tempfile::tempdir().unwrap();
        app.set_staging_parent(scratch.path().to_path_buf());
        let launched = Arc::new(Mutex::new(Vec::new()));
        let installed = Arc::new(Mutex::new(vec!["xterm".to_string()]));
        let seen = installed.clone();
        let (events, _source) = crate::processes::channel();
        let launcher = Launcher::custom(
            Box::new(FakeSpawner(launched.clone())),
            events,
            Platform::Linux,
            Box::new(move |name| {
                seen.lock().unwrap().iter().any(|candidate| candidate == name).then(|| name.to_string())
            }),
            None,
            scratch.path().to_path_buf(),
        );
        app.set_launcher(launcher);
        Harness { app, commands, start, scratch, launched, installed }
    }

    pub fn launched(&mut self) -> Vec<ProcessJob> {
        std::mem::take(&mut *self.launched.lock().unwrap())
    }

    pub fn finish_process(&mut self, token: u64, exit: porthmos_core::edit::EditorExit) {
        self.press(Message::ProcessFinished(ProcessEvent::Exited { token, exit }));
    }

    pub fn install_terminals(&mut self, names: &[&str]) {
        *self.installed.lock().unwrap() = names.iter().map(|name| name.to_string()).collect();
    }

    pub fn scratch(&self) -> &std::path::Path {
        self.scratch.path()
    }

    pub fn start_folder(&self) -> VfsPath {
        self.start.clone()
    }

    pub fn feed(&mut self, event: Event) {
        let _ = self.app.update(Message::Core(event));
    }

    pub fn press(&mut self, message: Message) {
        let _ = self.app.update(message);
    }

    pub fn sent(&mut self) -> Vec<Command> {
        let mut sent = Vec::new();
        while let Ok(command) = self.commands.try_recv() {
            sent.push(command);
        }
        sent
    }
}

#[derive(Clone, Default)]
pub struct NativeProbe {
    started: Arc<Mutex<Vec<Vec<PathBuf>>>>,
    cancels: Arc<AtomicUsize>,
    refusal: Arc<Mutex<Option<String>>>,
    events: EventLog,
}

impl NativeProbe {
    pub fn started(&self) -> Vec<Vec<PathBuf>> {
        self.started.lock().unwrap().clone()
    }

    pub fn cancels(&self) -> usize {
        self.cancels.load(Ordering::SeqCst)
    }

    pub fn refuse_with(&self, reason: &str) {
        *self.refusal.lock().unwrap() = Some(reason.to_string());
    }

    pub fn emit(&self, event: NativeEvent) {
        self.events.push(event);
    }
}

struct FakeNative(NativeProbe);

impl Native for FakeNative {
    fn start_drag(&mut self, paths: Vec<PathBuf>) -> Result<(), NativeError> {
        if let Some(reason) = self.0.refusal.lock().unwrap().clone() {
            return Err(NativeError(reason));
        }
        self.0.started.lock().unwrap().push(paths);
        Ok(())
    }

    fn cancel(&mut self) {
        self.0.cancels.fetch_add(1, Ordering::SeqCst);
    }

    fn poll(&mut self) -> Vec<NativeEvent> {
        self.0.events.take()
    }
}

impl Harness {
    pub fn attach_native(&mut self) -> NativeProbe {
        let probe = NativeProbe::default();
        let attached = Attached::new(Box::new(FakeNative(probe.clone())));
        self.press(Message::NativeAttached(Some(Arc::new(attached))));
        probe
    }
}

pub fn builtin_infos() -> Vec<ProtocolInfo> {
    let nowhere = Some("/nonexistent-porthmos-tests".into());
    let paths = porthmos_core::Paths::resolve(nowhere.clone(), nowhere, None).unwrap();
    porthmos_core::builtin_protocols(&paths)
        .iter()
        .map(|protocol| ProtocolInfo::from_protocol(protocol.as_ref()))
        .collect()
}

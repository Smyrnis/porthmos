use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use iced::futures::StreamExt;
use iced::futures::stream::unfold;
use iced::{Point, Subscription};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[cfg(target_os = "linux")]
pub mod uri;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(any(windows, target_os = "macos"))]
mod windows_macos;
#[cfg(target_os = "linux")]
mod x11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeEvent {
    Started,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeError(pub String);

impl std::fmt::Display for NativeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub trait Native: Send {
    fn start_drag(&mut self, paths: Vec<PathBuf>) -> Result<(), NativeError>;
    fn cancel(&mut self);
    fn poll(&mut self) -> Vec<NativeEvent>;
}

#[derive(Clone, Default)]
pub struct EventLog {
    events: Arc<Mutex<Vec<NativeEvent>>>,
}

impl EventLog {
    pub fn push(&self, event: NativeEvent) {
        self.events.lock().unwrap().push(event);
    }

    pub fn take(&self) -> Vec<NativeEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

pub struct Attached {
    native: Mutex<Box<dyn Native>>,
}

impl Attached {
    pub fn new(native: Box<dyn Native>) -> Attached {
        Attached { native: Mutex::new(native) }
    }

    pub fn start_drag(&self, paths: Vec<PathBuf>) -> Result<(), NativeError> {
        self.native.lock().unwrap().start_drag(paths)
    }

    pub fn cancel(&self) {
        self.native.lock().unwrap().cancel();
    }

    pub fn poll(&self) -> Vec<NativeEvent> {
        self.native.lock().unwrap().poll()
    }
}

impl std::fmt::Debug for Attached {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Attached")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DesktopEvent {
    Hover(Vec<PathBuf>),
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Moved(Point),
    Dropped {
        paths: Vec<PathBuf>,
        at: Option<Point>,
    },
    Left,
}

#[derive(Clone)]
pub struct DesktopSource {
    receiver: Arc<Mutex<Option<UnboundedReceiver<DesktopEvent>>>>,
}

impl DesktopSource {
    fn take(&self) -> Option<UnboundedReceiver<DesktopEvent>> {
        self.receiver.lock().unwrap().take()
    }

    pub fn subscription(&self) -> Subscription<DesktopEvent> {
        Subscription::run_with(self.clone(), |source| match source.take() {
            Some(receiver) => {
                unfold(receiver, |mut receiver| async move { receiver.recv().await.map(|event| (event, receiver)) })
                    .boxed()
            }
            None => iced::futures::stream::empty().boxed(),
        })
    }
}

impl Hash for DesktopSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "porthmos-desktop-events".hash(state);
    }
}

pub fn channel() -> (UnboundedSender<DesktopEvent>, DesktopSource) {
    let (sender, receiver) = unbounded_channel();
    (sender, DesktopSource { receiver: Arc::new(Mutex::new(Some(receiver))) })
}

#[cfg(target_os = "linux")]
pub fn attach(window: &dyn iced::window::Window, sender: UnboundedSender<DesktopEvent>) -> Option<Attached> {
    let native: Box<dyn Native> = match wayland::WaylandNative::attach(window, sender) {
        Some(native) => Box::new(native),
        None => Box::new(x11::X11Native::attach(window)?),
    };
    Some(Attached::new(native))
}

#[cfg(any(windows, target_os = "macos"))]
pub fn attach(window: &dyn iced::window::Window, _sender: UnboundedSender<DesktopEvent>) -> Option<Attached> {
    windows_macos::OsNative::attach(window).map(|native| Attached::new(Box::new(native)))
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub fn attach(_window: &dyn iced::window::Window, _sender: UnboundedSender<DesktopEvent>) -> Option<Attached> {
    None
}

use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use iced::Subscription;
use iced::futures::StreamExt;
use iced::futures::stream::unfold;
use porthmos_core::{Core, CoreHandle, Environment, Event, Paths, config};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::shortcuts::Shortcuts;
use crate::theme::ThemeSettings;

pub const SHUTDOWN_BOUND: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct EventSource {
    receiver: Arc<Mutex<Option<UnboundedReceiver<Event>>>>,
}

impl EventSource {
    pub fn new(receiver: UnboundedReceiver<Event>) -> Self {
        EventSource { receiver: Arc::new(Mutex::new(Some(receiver))) }
    }

    pub fn take(&self) -> Option<UnboundedReceiver<Event>> {
        self.receiver.lock().unwrap().take()
    }
}

impl Hash for EventSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "porthmos-core-events".hash(state);
    }
}

pub struct Started {
    pub core: CoreHandle,
    pub events: EventSource,
    pub warnings: Vec<String>,
    pub theme: ThemeSettings,
    pub shortcuts: Shortcuts,
}

pub fn start(env: Environment, paths: Paths) -> Result<Started> {
    paths.migrate_legacy_config();
    let (settings, config_warnings) = config::load(&paths)?;
    let (theme, theme_warnings) = ThemeSettings::from_frontend(&settings.frontend);
    let (shortcuts, key_warnings) = Shortcuts::from_frontend(&settings.frontend);
    let mut warnings: Vec<String> = config_warnings.into_iter().map(|warning| warning.0).collect();
    warnings.extend(theme_warnings);
    warnings.extend(key_warnings);
    let (core, receiver) =
        Core::builder().paths(paths).environment(env).settings(settings).release_updates().start()?;
    Ok(Started { core, events: EventSource::new(receiver), warnings, theme, shortcuts })
}

pub fn subscription(source: &EventSource) -> Subscription<Event> {
    Subscription::run_with(source.clone(), |source| match source.take() {
        Some(receiver) => {
            unfold(receiver, |mut receiver| async move { receiver.recv().await.map(|event| (event, receiver)) }).boxed()
        }
        None => iced::futures::stream::empty().boxed(),
    })
}

pub async fn bounded<F: Future>(future: F, limit: Duration) -> bool {
    tokio::time::timeout(limit, future).await.is_ok()
}

pub async fn shut_down(core: CoreHandle, limit: Duration) {
    let _ = bounded(async move { core.shutdown().await }, limit).await;
}

#[cfg(test)]
mod tests;

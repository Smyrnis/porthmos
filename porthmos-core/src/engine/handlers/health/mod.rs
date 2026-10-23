use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use porthmos_vfs::FileSystem;
use tokio::sync::mpsc::UnboundedSender;

use super::super::{Engine, Event, Internal, SessionId};
use crate::{
    Severity,
    health::{PROBE_INTERVAL, wait_unless_cancelled},
    tasks::Scope,
};

async fn watch(
    fs: Arc<dyn FileSystem>, session: SessionId, internal: UnboundedSender<Internal>, cancel: Arc<AtomicBool>,
) {
    let mut healthy = true;
    loop {
        let Some(alive) = fs.connection_alive().await else {
            return;
        };
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if alive != healthy {
            healthy = alive;
            let _ = internal.send(Internal::SessionHealth { session, healthy });
        }
        if !wait_unless_cancelled(PROBE_INTERVAL, &cancel).await {
            return;
        }
    }
}

impl Engine {
    pub(crate) fn start_health_probe(&mut self, session: SessionId) {
        let Some(live) = self.sessions.get(&session) else {
            return;
        };
        let (fs, internal) = (live.fs.clone(), self.internal.clone());
        self.tasks.spawn("health-probe", Scope::Session(session), move |cancel| watch(fs, session, internal, cancel));
    }

    pub(crate) fn stop_health_probe(&self, session: SessionId) {
        self.tasks.cancel(Scope::Session(session));
        self.tasks.forget(Scope::Session(session));
    }

    pub(crate) fn session_health_changed(&mut self, session: SessionId, healthy: bool) {
        let Some(live) = self.sessions.get(&session) else {
            return;
        };
        let name = live.name.clone();
        if healthy {
            self.info(format!("{name}: connection restored"));
        } else {
            self.report(Severity::Warning, format!("{name}: connection lost"));
        }
        self.emit(Event::SessionHealth { session, healthy });
    }
}

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use porthmos_tunnel::{RunningTunnel, StartError, StatsSnapshot, TunnelKey, TunnelSpec};
use tokio::sync::oneshot;

use super::super::{Engine, Event, Internal, SessionId, TunnelInfo, TunnelStatus};
use crate::{
    Message, Severity,
    config::tunnels::{self, TunnelsError, spec_error_text},
    health::wait_unless_cancelled,
    i18n,
    tasks::Scope,
};

const STATS_INTERVAL: Duration = Duration::from_secs(1);

enum Live {
    Starting { session: SessionId, start: u64, cancel: Arc<AtomicBool> },
    Running { session: SessionId, tunnel: RunningTunnel },
}

impl Live {
    fn session(&self) -> SessionId {
        match self {
            Live::Starting { session, .. } | Live::Running { session, .. } => *session,
        }
    }
}

#[derive(Default)]
pub(crate) struct TunnelRun {
    live: HashMap<TunnelKey, Live>,
    pending: HashMap<TunnelKey, oneshot::Receiver<()>>,
    outcome: HashMap<TunnelKey, TunnelStatus>,
    last_stats: HashMap<TunnelKey, StatsSnapshot>,
    next_start: u64,
    ticker: Option<Arc<AtomicBool>>,
}

impl Engine {
    fn session_for(&self, connection: &str) -> Option<SessionId> {
        self.book.sessions.iter().rev().find(|(_, live)| live.name == connection).map(|(&id, _)| id)
    }

    fn tunnel_status(&self, spec: &TunnelSpec) -> TunnelStatus {
        let key = spec.key();
        match self.tunnel_run.live.get(&key) {
            Some(Live::Starting { .. }) => TunnelStatus::Starting,
            Some(Live::Running { tunnel, .. }) => TunnelStatus::Running { local: tunnel.local_addr() },
            None => match self.tunnel_run.outcome.get(&key) {
                Some(outcome) => outcome.clone(),
                None if self.session_for(&spec.connection).is_some() => TunnelStatus::Stopped,
                None => TunnelStatus::Offline,
            },
        }
    }

    fn tunnel_info(&self, spec: &TunnelSpec) -> TunnelInfo {
        let key = spec.key();
        let stats = match self.tunnel_run.live.get(&key) {
            Some(Live::Running { tunnel, .. }) => tunnel.stats(),
            _ => self.tunnel_run.last_stats.get(&key).copied().unwrap_or_default(),
        };
        TunnelInfo { spec: spec.clone(), status: self.tunnel_status(spec), stats }
    }

    pub(crate) fn publish_tunnels(&self) {
        self.emit(Event::Tunnels(self.tunnels.iter().map(|spec| self.tunnel_info(spec)).collect()));
    }

    fn announce_tunnel(&self, key: &TunnelKey) {
        if let Some(spec) = self.tunnels.get(key) {
            self.emit(Event::TunnelStatus { key: key.clone(), status: self.tunnel_status(spec) });
        }
    }

    fn refuse_protected_tunnels(&self) -> bool {
        let protected = self.tunnels.is_protected();
        if protected {
            self.report(Severity::Error, i18n::text("tunnels.protected"));
        }
        protected
    }

    fn save_tunnels(&self) {
        if let Err(err) = tunnels::save_to(&self.host.paths.tunnels_file(), &self.tunnels) {
            self.report(Severity::Error, format!("{err:#}"));
        }
    }

    fn tunnel_failed(&mut self, key: &TunnelKey, message: Message) {
        self.tunnel_run.outcome.insert(key.clone(), TunnelStatus::Failed(message.clone()));
        self.report(Severity::Error, message);
        self.announce_tunnel(key);
    }

    pub(crate) fn save_tunnel(&mut self, original: Option<TunnelKey>, spec: TunnelSpec) {
        let key = spec.key();
        match self.tunnels.upsert(original.as_ref(), spec) {
            Ok(()) => {}
            Err(TunnelsError::Protected) => {
                self.refuse_protected_tunnels();
                return;
            }
            Err(TunnelsError::Duplicate) => {
                let values = [("name", key.name.as_str()), ("connection", key.connection.as_str())];
                self.report(Severity::Error, i18n::text_with("tunnels.duplicate", &values));
                return;
            }
            Err(TunnelsError::Invalid(error)) => {
                let values = [("name", key.name.as_str()), ("reason", &spec_error_text(error))];
                self.report(Severity::Error, i18n::text_with("tunnels.invalid", &values));
                return;
            }
        }
        if let Some(old) = original.filter(|old| *old != key) {
            self.rekey_tunnel(&old, &key);
        }
        self.save_tunnels();
        self.publish_tunnels();
    }

    fn rekey_tunnel(&mut self, old: &TunnelKey, new: &TunnelKey) {
        let run = &mut self.tunnel_run;
        match run.live.remove(old) {
            Some(running @ Live::Running { .. }) => {
                run.live.insert(new.clone(), running);
            }
            Some(Live::Starting { cancel, .. }) => cancel.store(true, Ordering::Relaxed),
            None => {}
        }
        if let Some(outcome) = run.outcome.remove(old) {
            run.outcome.insert(new.clone(), outcome);
        }
        if let Some(stats) = run.last_stats.remove(old) {
            run.last_stats.insert(new.clone(), stats);
        }
        if let Some(handle) = run.pending.remove(old) {
            run.pending.insert(new.clone(), handle);
        }
    }

    pub(crate) fn delete_tunnel(&mut self, key: &TunnelKey) {
        if self.tunnels.get(key).is_none() || self.refuse_protected_tunnels() {
            return;
        }
        self.stop_tunnel(key);
        self.tunnels.remove(key);
        let run = &mut self.tunnel_run;
        run.outcome.remove(key);
        run.last_stats.remove(key);
        run.pending.remove(key);
        self.save_tunnels();
        self.publish_tunnels();
    }

    pub(crate) fn start_tunnel(&mut self, key: &TunnelKey) {
        let Some(spec) = self.tunnels.get(key).cloned() else {
            let values = [("name", key.name.as_str()), ("connection", key.connection.as_str())];
            self.report(Severity::Warning, i18n::text_with("tunnels.unknown", &values));
            return;
        };
        if self.tunnel_run.live.contains_key(key) {
            return;
        }
        let Some(session) = self.session_for(&spec.connection) else {
            let values = [("connection", spec.connection.as_str()), ("name", spec.name.as_str())];
            self.report(Severity::Warning, i18n::text_with("tunnels.connect_first", &values));
            self.announce_tunnel(key);
            return;
        };
        let Some(forwarder) = self.book.sessions[&session].fs.forwarder() else {
            let message = Message::new(i18n::text_with("tunnels.cannot_forward", &[("name", &spec.connection)]));
            self.tunnel_failed(key, message);
            return;
        };

        let run = &mut self.tunnel_run;
        run.outcome.remove(key);
        run.last_stats.remove(key);
        run.next_start += 1;
        let start = run.next_start;
        let cancel = Arc::new(AtomicBool::new(false));
        run.live.insert(key.clone(), Live::Starting { session, start, cancel: cancel.clone() });
        let (finished, next) = oneshot::channel::<()>();
        let previous = run.pending.insert(key.clone(), next);
        let (internal, task_key) = (self.internal.clone(), key.clone());
        self.tasks.spawn("tunnel-start", Scope::Session(session), move |_| async move {
            let _finished = finished;
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let result = RunningTunnel::start(&spec, forwarder).await;
            if cancel.load(Ordering::Relaxed) {
                if let Ok(tunnel) = result {
                    tunnel.stop().await;
                }
                return;
            }
            let _ = internal.send(Internal::TunnelStarted { key: task_key, session, start, result });
        });
        self.announce_tunnel(key);
    }

    pub(crate) fn tunnel_started(
        &mut self, key: TunnelKey, session: SessionId, start: u64, result: Result<RunningTunnel, StartError>,
    ) {
        let current = matches!(
            self.tunnel_run.live.get(&key),
            Some(Live::Starting { session: waiting, start: number, .. }) if *waiting == session && *number == start
        );
        match result {
            Ok(tunnel) if !current => self.stop_in_background(&key, tunnel),
            Err(_) if !current => {}
            Ok(tunnel) => {
                let local = tunnel.local_addr().to_string();
                self.tunnel_run.live.insert(key.clone(), Live::Running { session, tunnel });
                self.start_stats_ticker();
                self.info(i18n::text_with("tunnels.listening", &[("name", &key.name), ("address", &local)]));
                self.announce_tunnel(&key);
            }
            Err(error) => {
                self.tunnel_run.live.remove(&key);
                let text = match error {
                    StartError::Invalid(error) => {
                        let values = [("name", key.name.as_str()), ("reason", &spec_error_text(error))];
                        i18n::text_with("tunnels.invalid", &values)
                    }
                    StartError::Bind { address, reason } => {
                        i18n::text_with("tunnels.bind_failed", &[("address", &address), ("reason", &reason)])
                    }
                    StartError::RemoteListen { address, reason } => {
                        i18n::text_with("tunnels.remote_listen_failed", &[("address", &address), ("reason", &reason)])
                    }
                };
                self.tunnel_failed(&key, Message::new(text));
            }
        }
    }

    fn stop_in_background(&mut self, key: &TunnelKey, tunnel: RunningTunnel) {
        let (finished, next) = oneshot::channel::<()>();
        let previous = self.tunnel_run.pending.insert(key.clone(), next);
        self.tasks.spawn("tunnel-stop", Scope::Background, move |_| async move {
            let _finished = finished;
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            tunnel.stop().await;
        });
    }

    fn end_live(&mut self, key: &TunnelKey, live: Live, outcome: TunnelStatus) {
        match live {
            Live::Running { tunnel, .. } => {
                self.tunnel_run.last_stats.insert(key.clone(), tunnel.stats());
                self.stop_in_background(key, tunnel);
            }
            Live::Starting { cancel, .. } => cancel.store(true, Ordering::Relaxed),
        }
        match outcome {
            TunnelStatus::Stopped => self.tunnel_run.outcome.remove(key),
            other => self.tunnel_run.outcome.insert(key.clone(), other),
        };
        self.announce_tunnel(key);
        self.stop_stats_ticker_when_idle();
    }

    pub(crate) fn stop_tunnel(&mut self, key: &TunnelKey) {
        if let Some(live) = self.tunnel_run.live.remove(key) {
            self.end_live(key, live, TunnelStatus::Stopped);
        }
    }

    pub(crate) fn restart_tunnel(&mut self, key: &TunnelKey) {
        self.stop_tunnel(key);
        self.start_tunnel(key);
    }

    pub(crate) fn stop_tunnels_of(&mut self, session: SessionId) {
        let keys: Vec<TunnelKey> = self
            .tunnel_run
            .live
            .iter()
            .filter(|(_, live)| live.session() == session)
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            if let Some(live) = self.tunnel_run.live.remove(&key) {
                self.end_live(&key, live, TunnelStatus::Lost);
            }
        }
        self.publish_tunnels();
    }

    pub(crate) fn tunnels_connected(&mut self, connection: &str) {
        let keys: Vec<TunnelKey> = self.tunnels.for_connection(connection).iter().map(|spec| spec.key()).collect();
        for key in &keys {
            if self.tunnel_run.outcome.get(key) == Some(&TunnelStatus::Lost) {
                self.tunnel_run.outcome.remove(key);
            }
        }
        let automatic: Vec<TunnelKey> =
            self.tunnels.for_connection(connection).iter().filter(|spec| spec.auto_start).map(|s| s.key()).collect();
        for key in automatic {
            self.start_tunnel(&key);
        }
        self.publish_tunnels();
    }

    pub(crate) fn tunnels_connection_renamed(&mut self, old: &str, new: &str) {
        if self.tunnels.for_connection(old).is_empty() || self.refuse_protected_tunnels() {
            return;
        }
        let keys: Vec<TunnelKey> = self.tunnels.for_connection(old).iter().map(|spec| spec.key()).collect();
        for key in keys {
            let renamed = TunnelKey { connection: new.to_string(), name: key.name.clone() };
            self.rekey_tunnel(&key, &renamed);
        }
        self.tunnels.rename_connection(old, new);
        self.save_tunnels();
        self.publish_tunnels();
    }

    pub(crate) fn tunnels_connection_deleted(&mut self, connection: &str) {
        if self.tunnels.for_connection(connection).is_empty() || self.refuse_protected_tunnels() {
            return;
        }
        for spec in self.tunnels.remove_connection(connection) {
            let key = spec.key();
            if let Some(live) = self.tunnel_run.live.remove(&key) {
                self.end_live(&key, live, TunnelStatus::Stopped);
            }
            let run = &mut self.tunnel_run;
            run.outcome.remove(&key);
            run.last_stats.remove(&key);
            run.pending.remove(&key);
        }
        self.save_tunnels();
        self.publish_tunnels();
    }

    fn start_stats_ticker(&mut self) {
        if self.tunnel_run.ticker.is_some() {
            return;
        }
        let internal = self.internal.clone();
        let cancel = self.tasks.spawn("tunnel-stats", Scope::Background, move |cancel| async move {
            while wait_unless_cancelled(STATS_INTERVAL, &cancel).await {
                if internal.send(Internal::TunnelTick).is_err() {
                    break;
                }
            }
        });
        self.tunnel_run.ticker = Some(cancel);
    }

    fn stop_stats_ticker_when_idle(&mut self) {
        let running = self.tunnel_run.live.values().any(|live| matches!(live, Live::Running { .. }));
        if !running && let Some(cancel) = self.tunnel_run.ticker.take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn tunnel_tick(&mut self) {
        let batch: Vec<(TunnelKey, StatsSnapshot)> = self
            .tunnel_run
            .live
            .iter()
            .filter_map(|(key, live)| match live {
                Live::Running { tunnel, .. } => Some((key.clone(), tunnel.stats())),
                Live::Starting { .. } => None,
            })
            .collect();
        for (key, stats) in &batch {
            self.tunnel_run.last_stats.insert(key.clone(), *stats);
        }
        if batch.is_empty() {
            self.stop_stats_ticker_when_idle();
        } else {
            self.emit(Event::TunnelStats(batch));
        }
    }
}

#[cfg(test)]
mod tests;

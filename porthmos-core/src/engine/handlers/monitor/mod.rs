use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use porthmos_monitor::{
    CommandError, POLL_TIMEOUT, PROCESS_ACTION_MAX_AGE, Parsed, SHELL_COMMAND, Sample, Token, may_signal, parse,
    probe_command, renice_command, signal_command,
};
use porthmos_vfs::{CommandOutput, CommandRunner, ProtocolError};
use tokio::{
    sync::mpsc::UnboundedSender,
    time::{Instant, timeout},
};

use super::{
    super::{Engine, Event, Internal, MonitorState, SessionId},
    FAILURES_BEFORE_NOTICE, Fetches, first_line, run_change,
};
use crate::{
    HealthSnapshot, Message, Nice, ProcessSignal, Severity, health::wait_unless_cancelled, i18n, tasks::Scope,
};

const PAIR_GAP: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(crate) enum MonitorUpdate {
    Snapshot { snapshot: Box<HealthSnapshot>, sample: Sample, at: Instant, last: bool },
    Unavailable(Message),
    Failed(String),
}

pub(crate) struct Taken {
    snapshot: HealthSnapshot,
    sample: Sample,
    at: Instant,
}

#[derive(Default)]
pub(crate) struct MonitorRun {
    fetches: Fetches<Taken>,
    cancels: HashMap<SessionId, Arc<AtomicBool>>,
}

enum Change {
    Signal(ProcessSignal),
    Priority(Nice),
}

fn judge(
    ran: Result<Result<CommandOutput, ProtocolError>, tokio::time::error::Elapsed>, token: &Token,
    previous: Option<&Sample>, elapsed: Duration, started: Instant, last: bool,
) -> MonitorUpdate {
    let output = match ran {
        Err(_) => return MonitorUpdate::Failed(i18n::text("monitor.reason_timeout")),
        Ok(Err(error)) => return MonitorUpdate::Failed(error.to_string()),
        Ok(Ok(output)) => output,
    };
    if output.status != Some(0) {
        return MonitorUpdate::Failed(
            first_line(&output.stderr).unwrap_or_else(|| i18n::text("monitor.reason_failed")),
        );
    }
    match parse(&output.stdout, token, previous, elapsed) {
        Parsed::Linux { snapshot, sample } => MonitorUpdate::Snapshot { snapshot, sample, at: started, last },
        Parsed::Unsupported { os } => {
            MonitorUpdate::Unavailable(Message::new(i18n::text_with("monitor.unavailable_not_linux", &[("os", &os)])))
        }
        Parsed::Garbled => MonitorUpdate::Failed(i18n::text("monitor.reason_unreadable")),
    }
}

async fn probe(runner: &Arc<dyn CommandRunner>, previous: Option<&(Sample, Instant)>, last: bool) -> MonitorUpdate {
    let token = Token::new();
    let started = Instant::now();
    let ran = timeout(POLL_TIMEOUT, runner.run_with_input(SHELL_COMMAND, probe_command(&token).as_bytes())).await;
    let elapsed = previous.map_or(Duration::ZERO, |(_, at)| started.duration_since(*at));
    judge(ran, &token, previous.map(|(sample, _)| sample), elapsed, started, last)
}

async fn fetch(
    runner: Arc<dyn CommandRunner>, session: SessionId, generation: u64, internal: UnboundedSender<Internal>,
    cancel: Arc<AtomicBool>, pair: bool, previous: Option<(Sample, Instant)>,
) {
    let mut previous = previous;
    let mut first_of_pair = pair;
    loop {
        let update = probe(&runner, previous.as_ref(), !first_of_pair).await;
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let carried = match &update {
            MonitorUpdate::Snapshot { sample, at, .. } if first_of_pair => Some((sample.clone(), *at)),
            _ => None,
        };
        if internal.send(Internal::Monitor { session, generation, update }).is_err() {
            return;
        }
        let Some(carried) = carried else {
            return;
        };
        previous = Some(carried);
        first_of_pair = false;
        if !wait_unless_cancelled(PAIR_GAP, &cancel).await {
            return;
        }
    }
}

impl Engine {
    pub(crate) fn start_monitor(&mut self, session: SessionId) {
        if self.monitor_run.fetches.has(session) {
            self.replay_monitor_unavailable(session);
            return;
        }
        self.fetch_monitor(session, true);
    }

    pub(crate) fn refresh_monitor(&mut self, session: SessionId, pair: bool) {
        if self.monitor_run.fetches.has(session) {
            self.fetch_monitor(session, pair);
        }
    }

    pub(crate) fn stop_monitor(&mut self, session: SessionId) {
        self.monitor_run.fetches.stop(session);
        if let Some(cancel) = self.monitor_run.cancels.remove(&session) {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    fn replay_monitor_unavailable(&mut self, session: SessionId) -> bool {
        let unavailable = self.monitor_run.fetches.slot(session).and_then(|slot| slot.unavailable.clone());
        let replayed = unavailable.is_some();
        if let Some(message) = unavailable {
            self.emit(Event::Monitor { session, state: MonitorState::Unavailable(message) });
        }
        replayed
    }

    fn fetch_monitor(&mut self, session: SessionId, pair: bool) {
        let Some(live) = self.book.sessions.get(&session) else {
            return;
        };
        let runner = live.fs.runner();
        if self.replay_monitor_unavailable(session) {
            return;
        }
        let Some(runner) = runner else {
            let message = Message::new(i18n::text("monitor.unavailable_no_runner"));
            self.emit(Event::Monitor { session, state: MonitorState::Unavailable(message) });
            return;
        };
        let Some(generation) = self.monitor_run.fetches.begin(session) else {
            return;
        };
        let previous = self
            .monitor_run
            .fetches
            .slot(session)
            .and_then(|slot| slot.latest.as_ref())
            .filter(|_| !pair)
            .map(|taken| (taken.sample.clone(), taken.at));
        let internal = self.internal.clone();
        let cancel = self.tasks.spawn("monitor-fetch", Scope::Session(session), move |cancel| {
            fetch(runner, session, generation, internal, cancel, pair, previous)
        });
        self.monitor_run.cancels.insert(session, cancel);
    }

    pub(crate) fn monitor_update(&mut self, session: SessionId, generation: u64, update: MonitorUpdate) {
        let fetches = &mut self.monitor_run.fetches;
        let slot = match &update {
            MonitorUpdate::Snapshot { last: false, .. } => fetches.current(session, generation),
            _ => fetches.finish(session, generation),
        };
        let Some(slot) = slot else {
            return;
        };
        let (state, notice) = match update {
            MonitorUpdate::Snapshot { snapshot, sample, at, last } => {
                if last {
                    slot.failures = 0;
                }
                slot.latest = Some(Taken { snapshot: (*snapshot).clone(), sample, at });
                (MonitorState::Snapshot(snapshot), None)
            }
            MonitorUpdate::Unavailable(message) => {
                slot.failures = 0;
                slot.latest = None;
                slot.unavailable = Some(message.clone());
                (MonitorState::Unavailable(message), None)
            }
            MonitorUpdate::Failed(reason) => {
                slot.failures += 1;
                let message = Message::new(i18n::text_with("monitor.failing", &[("reason", &reason)]));
                let notice = (slot.failures == FAILURES_BEFORE_NOTICE).then(|| message.clone());
                (MonitorState::Failing(message), notice)
            }
        };
        if let Some(message) = notice {
            self.report(Severity::Warning, message);
        }
        self.emit(Event::Monitor { session, state });
    }

    pub(crate) fn signal_process(&mut self, session: SessionId, pid: u32, name: &str, signal: ProcessSignal) {
        self.change_process(session, pid, name, Change::Signal(signal));
    }

    pub(crate) fn renice_process(&mut self, session: SessionId, pid: u32, name: &str, nice: Nice) {
        self.change_process(session, pid, name, Change::Priority(nice));
    }

    fn change_process(&mut self, session: SessionId, pid: u32, expected_name: &str, change: Change) {
        let taken = self.monitor_run.fetches.slot(session).and_then(|slot| slot.latest.as_ref());
        if taken.is_some_and(|taken| taken.at.elapsed() > PROCESS_ACTION_MAX_AGE) {
            self.report(Severity::Warning, i18n::text("monitor.list_is_old"));
            return;
        }
        let latest = taken.map(|taken| &taken.snapshot);
        let Some((owner, process)) = latest.and_then(|snapshot| {
            snapshot
                .processes
                .iter()
                .find(|process| process.pid == pid && process.name == expected_name)
                .map(|process| (snapshot.uid, process))
        }) else {
            self.report(Severity::Warning, i18n::text("monitor.unknown_process"));
            return;
        };
        let (name, pid_text) = (process.name.clone(), pid.to_string());
        let values = [("name", name.as_str()), ("pid", pid_text.as_str())];
        let command = match &change {
            Change::Signal(signal) => signal_command(pid, *signal),
            Change::Priority(nice) => renice_command(pid, *nice),
        };
        let command = match command {
            Ok(command) if may_signal(owner, process.uid) => command,
            Ok(_) | Err(CommandError::PidNotAllowed) => {
                self.report(Severity::Warning, i18n::text_with("monitor.not_yours", &values));
                return;
            }
        };
        let Some(runner) = self.book.sessions.get(&session).and_then(|live| live.fs.runner()) else {
            self.report(Severity::Warning, i18n::text("monitor.unavailable_no_runner"));
            return;
        };
        let success = match &change {
            Change::Signal(signal) => {
                let word = match signal {
                    ProcessSignal::Terminate => "TERM",
                    ProcessSignal::Kill => "KILL",
                };
                i18n::text_with("monitor.signal_sent", &[("signal", word), ("name", &name), ("pid", &pid_text)])
            }
            Change::Priority(nice) => i18n::text_with(
                "monitor.reniced",
                &[("name", &name), ("pid", &pid_text), ("nice", &nice.get().to_string())],
            ),
        };
        let gone = i18n::text_with("monitor.already_gone", &values);
        let reporter = self.reporter.clone();
        self.tasks.spawn("monitor-change", Scope::Background, move |_| async move {
            let (severity, text) = match run_change(runner, command).await {
                Ok(output) if output.status == Some(0) => (Severity::Info, success),
                Ok(output) if String::from_utf8_lossy(&output.stderr).to_lowercase().contains("no such process") => {
                    (Severity::Info, gone)
                }
                Ok(output) => {
                    let reason = first_line(&output.stderr).unwrap_or_else(|| i18n::text("monitor.reason_failed"));
                    let values = [("name", name.as_str()), ("pid", pid_text.as_str()), ("reason", reason.as_str())];
                    (Severity::Error, i18n::text_with("monitor.signal_failed", &values))
                }
                Err(reason) => {
                    let values = [("name", name.as_str()), ("pid", pid_text.as_str()), ("reason", reason.as_str())];
                    (Severity::Error, i18n::text_with("monitor.signal_failed", &values))
                }
            };
            reporter.report(severity, Message::new(text));
        });
    }
}

#[cfg(test)]
mod tests;

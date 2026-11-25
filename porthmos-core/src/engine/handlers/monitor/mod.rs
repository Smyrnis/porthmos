use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use porthmos_monitor::{
    CommandError, POLL_TIMEOUT, Parsed, SHELL_COMMAND, Sample, Token, change_script, may_signal, parse, probe_command,
    renice_command, signal_command,
};
use porthmos_vfs::{CommandOutput, CommandRunner, ProtocolError};
use tokio::{
    sync::mpsc::UnboundedSender,
    time::{Instant, timeout},
};

use super::super::{Engine, Event, Internal, MonitorState, SessionId};
use crate::{
    HealthSnapshot, Message, Nice, POLL_INTERVAL, ProcessSignal, Severity, health::wait_unless_cancelled, i18n,
    tasks::Scope,
};

const FAILURES_BEFORE_FAILING: u32 = 3;
const FAILING_INTERVAL: Duration = Duration::from_secs(30);
const REASON_LIMIT: usize = 200;

#[derive(Debug)]
pub(crate) enum MonitorUpdate {
    Snapshot(Box<HealthSnapshot>),
    Unavailable(Message),
    Failing(Message),
}

struct Running {
    generation: u64,
    cancel: Arc<AtomicBool>,
    latest: Option<HealthSnapshot>,
    unavailable: Option<Message>,
}

#[derive(Default)]
pub(crate) struct MonitorRun {
    sessions: HashMap<SessionId, Running>,
    next_generation: u64,
}

enum Outcome {
    Good(Box<HealthSnapshot>, Sample),
    Unavailable(Message),
    Failed(String),
}

enum Change {
    Signal(ProcessSignal),
    Priority(Nice),
}

fn first_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| line.chars().filter(|character| !character.is_control()).collect::<String>())
        .map(|line| line.trim().chars().take(REASON_LIMIT).collect::<String>())
        .find(|line| !line.is_empty())
}

fn judge(
    ran: Result<Result<CommandOutput, ProtocolError>, tokio::time::error::Elapsed>, token: &Token,
    previous: Option<&Sample>, elapsed: Duration,
) -> Outcome {
    let output = match ran {
        Err(_) => return Outcome::Failed(i18n::text("monitor.reason_timeout")),
        Ok(Err(error)) => return Outcome::Failed(error.to_string()),
        Ok(Ok(output)) => output,
    };
    if output.status != Some(0) {
        return Outcome::Failed(first_line(&output.stderr).unwrap_or_else(|| i18n::text("monitor.reason_failed")));
    }
    match parse(&output.stdout, token, previous, elapsed) {
        Parsed::Linux { snapshot, sample } => Outcome::Good(snapshot, sample),
        Parsed::Unsupported { os } => {
            Outcome::Unavailable(Message::new(i18n::text_with("monitor.unavailable_not_linux", &[("os", &os)])))
        }
        Parsed::Garbled => Outcome::Failed(i18n::text("monitor.reason_unreadable")),
    }
}

async fn poll(
    runner: Arc<dyn CommandRunner>, session: SessionId, generation: u64, internal: UnboundedSender<Internal>,
    cancel: Arc<AtomicBool>,
) {
    let mut sample: Option<Sample> = None;
    let mut sampled_at: Option<Instant> = None;
    let mut failures = 0u32;
    loop {
        let token = Token::new();
        let started = Instant::now();
        let ran = timeout(POLL_TIMEOUT, runner.run_with_input(SHELL_COMMAND, probe_command(&token).as_bytes())).await;
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let elapsed = sampled_at.map_or(Duration::ZERO, |at| started.duration_since(at));
        let update = match judge(ran, &token, sample.as_ref(), elapsed) {
            Outcome::Good(snapshot, next) => {
                failures = 0;
                sample = Some(next);
                sampled_at = Some(started);
                Some(MonitorUpdate::Snapshot(snapshot))
            }
            Outcome::Unavailable(message) => {
                let _ = internal.send(Internal::Monitor {
                    session,
                    generation,
                    update: MonitorUpdate::Unavailable(message),
                });
                return;
            }
            Outcome::Failed(reason) => {
                failures += 1;
                (failures == FAILURES_BEFORE_FAILING).then(|| {
                    MonitorUpdate::Failing(Message::new(i18n::text_with("monitor.failing", &[("reason", &reason)])))
                })
            }
        };
        if let Some(update) = update
            && internal.send(Internal::Monitor { session, generation, update }).is_err()
        {
            return;
        }
        let pause = if failures >= FAILURES_BEFORE_FAILING { FAILING_INTERVAL } else { POLL_INTERVAL };
        if !wait_unless_cancelled(pause, &cancel).await {
            return;
        }
    }
}

async fn run_change(runner: Arc<dyn CommandRunner>, command: String) -> Result<CommandOutput, String> {
    match timeout(POLL_TIMEOUT, runner.run_with_input(SHELL_COMMAND, change_script(&command).as_bytes())).await {
        Err(_) => Err(i18n::text("monitor.reason_timeout")),
        Ok(Err(error)) => Err(error.to_string()),
        Ok(Ok(output)) => Ok(output),
    }
}

impl Engine {
    pub(crate) fn start_monitor(&mut self, session: SessionId) {
        let Some(live) = self.book.sessions.get(&session) else {
            return;
        };
        if let Some(running) = self.monitor_run.sessions.get(&session) {
            if let Some(message) = running.unavailable.clone() {
                self.emit(Event::Monitor { session, state: MonitorState::Unavailable(message) });
            }
            return;
        }
        let Some(runner) = live.fs.runner() else {
            let message = Message::new(i18n::text("monitor.unavailable_no_runner"));
            self.emit(Event::Monitor { session, state: MonitorState::Unavailable(message) });
            return;
        };
        let generation = self.monitor_run.next_generation;
        self.monitor_run.next_generation += 1;
        let internal = self.internal.clone();
        let cancel = self.tasks.spawn("monitor-poll", Scope::Session(session), move |cancel| {
            poll(runner, session, generation, internal, cancel)
        });
        self.monitor_run.sessions.insert(session, Running { generation, cancel, latest: None, unavailable: None });
    }

    pub(crate) fn stop_monitor(&mut self, session: SessionId) {
        if let Some(running) = self.monitor_run.sessions.remove(&session) {
            running.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn monitor_update(&mut self, session: SessionId, generation: u64, update: MonitorUpdate) {
        let Some(running) = self.monitor_run.sessions.get_mut(&session).filter(|run| run.generation == generation)
        else {
            return;
        };
        let state = match update {
            MonitorUpdate::Snapshot(snapshot) => {
                running.latest = Some((*snapshot).clone());
                MonitorState::Snapshot(snapshot)
            }
            MonitorUpdate::Unavailable(message) => {
                running.latest = None;
                running.unavailable = Some(message.clone());
                MonitorState::Unavailable(message)
            }
            MonitorUpdate::Failing(message) => {
                self.report(Severity::Warning, message.clone());
                MonitorState::Failing(message)
            }
        };
        self.emit(Event::Monitor { session, state });
    }

    pub(crate) fn signal_process(&mut self, session: SessionId, pid: u32, name: &str, signal: ProcessSignal) {
        self.change_process(session, pid, name, Change::Signal(signal));
    }

    pub(crate) fn renice_process(&mut self, session: SessionId, pid: u32, name: &str, nice: Nice) {
        self.change_process(session, pid, name, Change::Priority(nice));
    }

    fn change_process(&mut self, session: SessionId, pid: u32, expected_name: &str, change: Change) {
        let latest = self.monitor_run.sessions.get(&session).and_then(|running| running.latest.as_ref());
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

use std::sync::Arc;

use porthmos_monitor::{POLL_TIMEOUT, ParsedPorts, PortsSnapshot, SHELL_COMMAND, Token, parse_ports, ports_command};
use porthmos_vfs::{CommandOutput, CommandRunner, ProtocolError};
use tokio::{sync::mpsc::UnboundedSender, time::timeout};

use super::{
    super::{Engine, Event, Internal, PortsState, SessionId},
    FAILURES_BEFORE_NOTICE, first_line,
};
use crate::{Message, Severity, i18n, tasks::Scope};

#[derive(Debug)]
pub(crate) enum PortsUpdate {
    Snapshot(Box<PortsSnapshot>),
    Unavailable(Message),
    Failed(String),
}

fn judge(ran: Result<Result<CommandOutput, ProtocolError>, tokio::time::error::Elapsed>, token: &Token) -> PortsUpdate {
    let output = match ran {
        Err(_) => return PortsUpdate::Failed(i18n::text("ports.reason_timeout")),
        Ok(Err(error)) => return PortsUpdate::Failed(error.to_string()),
        Ok(Ok(output)) => output,
    };
    if output.status != Some(0) {
        return PortsUpdate::Failed(first_line(&output.stderr).unwrap_or_else(|| i18n::text("ports.reason_failed")));
    }
    match parse_ports(&output.stdout, token) {
        ParsedPorts::Linux(snapshot) => PortsUpdate::Snapshot(snapshot),
        ParsedPorts::Unsupported { os } => {
            PortsUpdate::Unavailable(Message::new(i18n::text_with("ports.unavailable_not_linux", &[("os", &os)])))
        }
        ParsedPorts::Garbled => PortsUpdate::Failed(i18n::text("ports.reason_unreadable")),
    }
}

async fn fetch(
    runner: Arc<dyn CommandRunner>, session: SessionId, generation: u64, internal: UnboundedSender<Internal>,
) {
    let token = Token::new();
    let ran = timeout(POLL_TIMEOUT, runner.run_with_input(SHELL_COMMAND, ports_command(&token).as_bytes())).await;
    let _ = internal.send(Internal::Ports { session, generation, update: judge(ran, &token) });
}

impl Engine {
    pub(crate) fn start_ports(&mut self, session: SessionId) {
        if self.ports_run.has(session) {
            self.replay_ports_unavailable(session);
            return;
        }
        self.fetch_ports(session);
    }

    pub(crate) fn refresh_ports(&mut self, session: SessionId) {
        if self.ports_run.has(session) {
            self.fetch_ports(session);
        }
    }

    pub(crate) fn stop_ports(&mut self, session: SessionId) {
        self.ports_run.stop(session);
    }

    fn replay_ports_unavailable(&mut self, session: SessionId) -> bool {
        let unavailable = self.ports_run.slot(session).and_then(|slot| slot.unavailable.clone());
        let replayed = unavailable.is_some();
        if let Some(message) = unavailable {
            self.emit(Event::Ports { session, state: PortsState::Unavailable(message) });
        }
        replayed
    }

    fn fetch_ports(&mut self, session: SessionId) {
        let Some(live) = self.book.sessions.get(&session) else {
            return;
        };
        let runner = live.fs.runner();
        if self.replay_ports_unavailable(session) {
            return;
        }
        let Some(runner) = runner else {
            let message = Message::new(i18n::text("ports.unavailable_no_runner"));
            self.emit(Event::Ports { session, state: PortsState::Unavailable(message) });
            return;
        };
        let Some(generation) = self.ports_run.begin(session) else {
            return;
        };
        let internal = self.internal.clone();
        self.tasks.spawn("ports-fetch", Scope::Session(session), move |_| fetch(runner, session, generation, internal));
    }

    pub(crate) fn ports_update(&mut self, session: SessionId, generation: u64, update: PortsUpdate) {
        let Some(slot) = self.ports_run.finish(session, generation) else {
            return;
        };
        let (state, notice) = match update {
            PortsUpdate::Snapshot(snapshot) => {
                slot.failures = 0;
                (PortsState::Snapshot(snapshot), None)
            }
            PortsUpdate::Unavailable(message) => {
                slot.failures = 0;
                slot.unavailable = Some(message.clone());
                (PortsState::Unavailable(message), None)
            }
            PortsUpdate::Failed(reason) => {
                slot.failures += 1;
                let message = Message::new(i18n::text_with("ports.failing", &[("reason", &reason)]));
                let notice = (slot.failures == FAILURES_BEFORE_NOTICE).then(|| message.clone());
                (PortsState::Failing(message), notice)
            }
        };
        if let Some(message) = notice {
            self.report(Severity::Warning, message);
        }
        self.emit(Event::Ports { session, state });
    }
}

#[cfg(test)]
mod tests;

use std::sync::Arc;

use porthmos_monitor::{
    POLL_TIMEOUT, ParsedServices, Privilege, SHELL_COMMAND, ServiceVerb, ServicesSnapshot, SudoProblem, Token, Unit,
    UnitName, hides_or_reorders_text, parse_services, service_command, services_command, status_command, sudo_problem,
    verb_allowed,
};
use porthmos_vfs::{CommandOutput, CommandRunner, ProtocolError};
use tokio::{sync::mpsc::UnboundedSender, time::timeout};

use super::{
    super::{Engine, Event, Internal, ServicesState, SessionId},
    FAILURES_BEFORE_NOTICE, first_line, run_change,
};
use crate::{Message, Severity, i18n, tasks::Scope};

const STATUS_LIMIT: usize = 16 * 1024;
const SHOWN_NAME_LIMIT: usize = 100;

#[derive(Debug)]
pub(crate) enum ServicesUpdate {
    Snapshot(Box<ServicesSnapshot>),
    Unavailable(Message),
    Failed(String),
}

fn judge(
    ran: Result<Result<CommandOutput, ProtocolError>, tokio::time::error::Elapsed>, token: &Token,
) -> ServicesUpdate {
    let output = match ran {
        Err(_) => return ServicesUpdate::Failed(i18n::text("services.reason_timeout")),
        Ok(Err(error)) => return ServicesUpdate::Failed(error.to_string()),
        Ok(Ok(output)) => output,
    };
    if output.status != Some(0) {
        return ServicesUpdate::Failed(
            first_line(&output.stderr).unwrap_or_else(|| i18n::text("services.reason_failed")),
        );
    }
    match parse_services(&output.stdout, token) {
        ParsedServices::Linux(snapshot) => ServicesUpdate::Snapshot(snapshot),
        ParsedServices::NoSystemd => {
            ServicesUpdate::Unavailable(Message::new(i18n::text("services.unavailable_no_systemd")))
        }
        ParsedServices::Unsupported { os } => {
            ServicesUpdate::Unavailable(Message::new(i18n::text_with("services.unavailable_not_linux", &[("os", &os)])))
        }
        ParsedServices::Garbled => ServicesUpdate::Failed(i18n::text("services.reason_unreadable")),
    }
}

async fn fetch(
    runner: Arc<dyn CommandRunner>, session: SessionId, generation: u64, internal: UnboundedSender<Internal>,
) {
    let token = Token::new();
    let ran = timeout(POLL_TIMEOUT, runner.run_with_input(SHELL_COMMAND, services_command(&token).as_bytes())).await;
    let _ = internal.send(Internal::Services { session, generation, update: judge(ran, &token) });
}

fn clean_status(bytes: &[u8]) -> String {
    let mut text = String::new();
    for character in String::from_utf8_lossy(bytes).chars().filter(|character| {
        (!character.is_control() || matches!(character, '\n' | '\t')) && !hides_or_reorders_text(*character)
    }) {
        if text.len() + character.len_utf8() > STATUS_LIMIT {
            break;
        }
        text.push(character);
    }
    text
}

fn shown_name(name: &str) -> String {
    name.chars()
        .filter(|character| !character.is_control() && !hides_or_reorders_text(*character))
        .take(SHOWN_NAME_LIMIT)
        .collect()
}

fn action_failure(name: &str, privilege: Privilege, output: &CommandOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let problem = (privilege == Privilege::Sudo).then(|| sudo_problem(&stderr)).flatten();
    let key = match problem {
        Some(SudoProblem::NeedsPassword) => "services.needs_password",
        Some(SudoProblem::NotAllowed) => "services.sudo_not_allowed",
        Some(SudoProblem::NotInstalled) => "services.sudo_not_installed",
        None => {
            let reason = first_line(&output.stderr).unwrap_or_else(|| i18n::text("services.reason_failed"));
            return i18n::text_with("services.failed", &[("name", name), ("reason", &reason)]);
        }
    };
    i18n::text_with(key, &[("name", name)])
}

impl Engine {
    pub(crate) fn start_services(&mut self, session: SessionId) {
        if self.services_run.has(session) {
            self.replay_services_unavailable(session);
            return;
        }
        self.fetch_services(session);
    }

    pub(crate) fn refresh_services(&mut self, session: SessionId) {
        if self.services_run.has(session) {
            self.fetch_services(session);
        }
    }

    pub(crate) fn stop_services(&mut self, session: SessionId) {
        self.services_run.stop(session);
    }

    fn replay_services_unavailable(&mut self, session: SessionId) -> bool {
        let unavailable = self.services_run.slot(session).and_then(|slot| slot.unavailable.clone());
        let replayed = unavailable.is_some();
        if let Some(message) = unavailable {
            self.emit(Event::Services { session, state: ServicesState::Unavailable(message) });
        }
        replayed
    }

    fn fetch_services(&mut self, session: SessionId) {
        let Some(live) = self.book.sessions.get(&session) else {
            return;
        };
        let runner = live.fs.runner();
        if self.replay_services_unavailable(session) {
            return;
        }
        let Some(runner) = runner else {
            let message = Message::new(i18n::text("services.unavailable_no_runner"));
            self.emit(Event::Services { session, state: ServicesState::Unavailable(message) });
            return;
        };
        let Some(generation) = self.services_run.begin(session) else {
            return;
        };
        let internal = self.internal.clone();
        self.tasks
            .spawn("services-fetch", Scope::Session(session), move |_| fetch(runner, session, generation, internal));
    }

    pub(crate) fn services_update(&mut self, session: SessionId, generation: u64, update: ServicesUpdate) {
        let Some(slot) = self.services_run.finish(session, generation) else {
            return;
        };
        let (state, notice) = match update {
            ServicesUpdate::Snapshot(snapshot) => {
                slot.failures = 0;
                slot.latest = Some((*snapshot).clone());
                (ServicesState::Snapshot(snapshot), None)
            }
            ServicesUpdate::Unavailable(message) => {
                slot.failures = 0;
                slot.latest = None;
                slot.unavailable = Some(message.clone());
                (ServicesState::Unavailable(message), None)
            }
            ServicesUpdate::Failed(reason) => {
                slot.failures += 1;
                let message = Message::new(i18n::text_with("services.failing", &[("reason", &reason)]));
                let notice = (slot.failures == FAILURES_BEFORE_NOTICE).then(|| message.clone());
                (ServicesState::Failing(message), notice)
            }
        };
        let refetch = self.services_run.take_refetch(session) && !matches!(state, ServicesState::Unavailable(_));
        if let Some(message) = notice {
            self.report(Severity::Warning, message);
        }
        self.emit(Event::Services { session, state });
        if refetch {
            self.fetch_services(session);
        }
    }

    pub(crate) fn service_changed(&mut self, session: SessionId) {
        if self.services_run.has(session) && !self.services_run.queue_refetch(session) {
            self.fetch_services(session);
        }
    }

    fn listed_unit(&mut self, session: SessionId, name: &str) -> Option<(Unit, u32, UnitName)> {
        let snapshot = self.services_run.slot(session).and_then(|slot| slot.latest.as_ref());
        let shown = shown_name(name);
        let Some((unit, uid)) = snapshot.and_then(|snapshot| {
            snapshot.units.iter().find(|unit| unit.name == name).map(|u| (u.clone(), snapshot.uid))
        }) else {
            self.report(Severity::Warning, i18n::text_with("services.unknown_unit", &[("name", &shown)]));
            return None;
        };
        let Some(checked) = UnitName::new(&unit.name) else {
            self.report(Severity::Warning, i18n::text_with("services.unsafe_name", &[("name", &shown)]));
            return None;
        };
        Some((unit, uid, checked))
    }

    pub(crate) fn service_action(&mut self, session: SessionId, name: &str, verb: ServiceVerb) {
        let Some((unit, uid, checked)) = self.listed_unit(session, name) else {
            return;
        };
        if !verb_allowed(&unit, verb) {
            self.report(Severity::Warning, i18n::text_with("services.not_possible", &[("name", name)]));
            return;
        }
        let Some(runner) = self.book.sessions.get(&session).and_then(|live| live.fs.runner()) else {
            self.report(Severity::Warning, i18n::text("services.unavailable_no_runner"));
            return;
        };
        let privilege = Privilege::for_uid(uid);
        let command = service_command(verb, &checked, privilege);
        let success_key = match verb {
            ServiceVerb::Start => "services.started",
            ServiceVerb::Stop => "services.stopped",
            ServiceVerb::Restart => "services.restarted",
            ServiceVerb::Enable => "services.enabled",
            ServiceVerb::Disable => "services.disabled",
        };
        let name = name.to_string();
        let success = i18n::text_with(success_key, &[("name", &name)]);
        let reporter = self.reporter.clone();
        let internal = self.internal.clone();
        self.tasks.spawn("service-action", Scope::Background, move |_| async move {
            let (severity, text) = match run_change(runner, command).await {
                Ok(output) if output.status == Some(0) => (Severity::Info, success),
                Ok(output) => (Severity::Error, action_failure(&name, privilege, &output)),
                Err(reason) => {
                    (Severity::Error, i18n::text_with("services.failed", &[("name", &name), ("reason", &reason)]))
                }
            };
            reporter.report(severity, Message::new(text));
            let _ = internal.send(Internal::ServiceChanged { session });
        });
    }

    pub(crate) fn service_status(&mut self, session: SessionId, name: &str) {
        let Some((_, _, checked)) = self.listed_unit(session, name) else {
            return;
        };
        let Some(runner) = self.book.sessions.get(&session).and_then(|live| live.fs.runner()) else {
            self.report(Severity::Warning, i18n::text("services.unavailable_no_runner"));
            return;
        };
        let command = status_command(&checked);
        let name = name.to_string();
        let internal = self.internal.clone();
        self.tasks.spawn("service-status", Scope::Session(session), move |_| async move {
            let text = match run_change(runner, command).await {
                Ok(output) => {
                    let text = clean_status(&output.stdout);
                    if text.trim().is_empty() { clean_status(&output.stderr) } else { text }
                }
                Err(reason) => i18n::text_with("services.status_failed", &[("name", &name), ("reason", &reason)]),
            };
            let _ = internal.send(Internal::ServiceStatus { session, unit: name, text });
        });
    }

    pub(crate) fn service_status_ready(&mut self, session: SessionId, unit: String, text: String) {
        if self.services_run.has(session) {
            self.emit(Event::ServiceStatus { session, unit, text });
        }
    }
}

#[cfg(test)]
mod tests;

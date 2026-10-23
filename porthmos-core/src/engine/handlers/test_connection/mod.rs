mod prompter;

use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use porthmos_vfs::{OptionKind, Protocol, Target};
use prompter::{Shared, TestPrompter};
use tokio::time::Instant;

use super::{connect::secret_account, profiles::profile_account};
use crate::{
    TestOutcome, TestReport, TestStage,
    engine::{Engine, Event},
    profiles::{ConnectionEntry, PASSWORD_MARKER, ProfileDraft, SecretEdit},
    secrets::Secrets,
    tasks::Scope,
    test_connection::{TEST_LIMIT, check_start_folder, stage_of},
};

const TICK: Duration = Duration::from_millis(50);
const REMOTE_PATH: &str = "remote_path";

enum Source {
    Value(String),
    Saved(String),
    Nothing,
}

struct SecretPlan {
    password: Source,
    options: Vec<(&'static str, Source)>,
}

enum Interrupt {
    Cancelled,
    TimedOut(TestStage),
}

fn source_of(edit: &SecretEdit, saved_account: Option<String>) -> Source {
    match (edit, saved_account) {
        (SecretEdit::Replace(value), _) => Source::Value(value.clone()),
        (SecretEdit::Keep, Some(account)) => Source::Saved(account),
        _ => Source::Nothing,
    }
}

async fn resolve(source: &Source, secrets: &Secrets) -> Option<String> {
    match source {
        Source::Value(value) => Some(value.clone()),
        Source::Saved(account) => secrets.lookup(account, true).await.map(|secret| secret.to_string()),
        Source::Nothing => None,
    }
}

async fn bounded<T>(
    shared: &Shared, cancel: &AtomicBool, started: Instant, stage: impl Fn() -> TestStage,
    future: impl Future<Output = T>,
) -> Result<T, Interrupt> {
    tokio::pin!(future);
    loop {
        tokio::select! {
            result = &mut future => return Ok(result),
            _ = tokio::time::sleep(TICK) => {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Interrupt::Cancelled);
                }
                if started.elapsed().saturating_sub(shared.waiting_total()) >= TEST_LIMIT {
                    return Err(Interrupt::TimedOut(stage()));
                }
            }
        }
    }
}

struct Attempt {
    protocol: Arc<dyn Protocol>,
    target: Target,
    plan: SecretPlan,
    start_path: Option<String>,
    secrets: Secrets,
}

async fn run(attempt: Attempt, mut prompter: TestPrompter, cancel: Arc<AtomicBool>) -> TestOutcome {
    let Attempt { protocol, mut target, plan, start_path, secrets } = attempt;
    target.password = resolve(&plan.password, &secrets).await;
    for (key, source) in &plan.options {
        if let Some(value) = resolve(source, &secrets).await {
            target.options.insert((*key).to_string(), value);
        }
    }

    let shared = prompter.shared.clone();
    let started = Instant::now();
    let stage = || if shared.login_started() { TestStage::Login } else { TestStage::Connect };
    let connected = bounded(&shared, &cancel, started, stage, protocol.connect(&target, &mut prompter)).await;
    let filesystem = match connected {
        Err(Interrupt::Cancelled) => return TestOutcome::Cancelled,
        Err(Interrupt::TimedOut(stage)) => return TestOutcome::TimedOut(stage),
        Ok(Err(error)) => {
            if let Some(untrusted) = prompter.untrusted.take() {
                return TestOutcome::NotTrusted(untrusted);
            }
            return match stage_of(error.kind()) {
                None => TestOutcome::Cancelled,
                Some(stage) => TestOutcome::Failed {
                    stage,
                    message: crate::connect_failure_message(&error, &target.name, protocol.display_name()),
                },
            };
        }
        Ok(Ok(filesystem)) => filesystem,
    };

    let checked = bounded(
        &shared,
        &cancel,
        started,
        || TestStage::StartFolder,
        check_start_folder(filesystem.as_ref(), start_path.as_deref()),
    )
    .await;
    let start_folder = match checked {
        Err(Interrupt::Cancelled) => return TestOutcome::Cancelled,
        Err(Interrupt::TimedOut(stage)) => return TestOutcome::TimedOut(stage),
        Ok(folder) => folder,
    };
    let elapsed = started.elapsed().saturating_sub(shared.waiting_total());
    let server = filesystem.server_info();
    drop(filesystem);
    TestOutcome::Connected(TestReport { elapsed, server, start_folder })
}

impl Engine {
    pub(crate) fn cancel_test(&mut self) {
        self.tasks.cancel(Scope::Test);
        self.tasks.forget(Scope::Test);
    }

    pub(crate) fn test_connection(&mut self, test_id: u64, original: Option<String>, draft: ProfileDraft) {
        self.cancel_test();
        let events = self.events.clone();
        let reject = |message: String| {
            let _ = events.send(Event::TestFinished {
                test_id,
                outcome: TestOutcome::Failed { stage: TestStage::Settings, message },
            });
        };
        let Some(protocol) = self.protocols.iter().find(|protocol| protocol.id() == draft.protocol).cloned() else {
            return reject(format!("No \"{}\" protocol is available", draft.protocol));
        };
        let preserve_from: Option<ConnectionEntry> = original
            .as_ref()
            .and_then(|name| self.all_profiles().ok()?.into_iter().find(|entry| &entry.name == name))
            .filter(|entry| entry.protocol == draft.protocol);
        let form = protocol.connection_form();
        let profile = match draft.validate(&form, preserve_from.as_ref()) {
            Ok(profile) => profile,
            Err(message) => return reject(message),
        };

        let saved = |marker: &str| preserve_from.as_ref().filter(|entry| entry.in_keyring.iter().any(|m| m == marker));
        let password_account = saved(PASSWORD_MARKER).map(secret_account);
        let options = form
            .options
            .iter()
            .filter(|field| field.kind == OptionKind::Secret)
            .map(|field| {
                let edit = draft.secret_options.get(field.key).cloned().unwrap_or_default();
                let account = saved(field.key).map(|entry| profile_account(&entry.name, field.key));
                (field.key, source_of(&edit, account))
            })
            .collect();
        let plan = SecretPlan { password: source_of(&draft.password, password_account), options };
        let start_path = profile.options.get(REMOTE_PATH).map(|path| path.trim().to_string()).filter(|p| !p.is_empty());
        let target = Target {
            name: profile.name.clone(),
            host: profile.host.clone(),
            port: profile.port.unwrap_or(form.port.default),
            username: profile.username.clone(),
            password: None,
            options: profile.options.clone(),
        };
        let attempt = Attempt { protocol, target, plan, start_path, secrets: self.secrets.clone() };
        let prompter = TestPrompter {
            questions: self.questions.clone(),
            events: self.events.clone(),
            test_id,
            shared: Arc::default(),
            untrusted: None,
        };
        let finished = self.events.clone();
        self.tasks.spawn("connection-test", Scope::Test, move |cancel| async move {
            let outcome = run(attempt, prompter, cancel).await;
            let _ = finished.send(Event::TestFinished { test_id, outcome });
        });
    }
}

#[cfg(test)]
mod tests;

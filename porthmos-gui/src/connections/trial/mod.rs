use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::{NotTrusted, RequestId, StartFolder, TestOutcome, TestStage};

use crate::dialogs::Typed;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestState {
    Running,
    Finished(TestOutcome),
}

#[derive(Debug, Clone)]
pub struct TestPrompt {
    pub request_id: RequestId,
    pub username: String,
    pub name: String,
    pub secret: &'static str,
    pub text: Typed,
}

impl TestPrompt {
    pub fn heading(&self) -> String {
        i18n::text_with(
            "gui.trial.heading",
            &[("secret", &tr(self.secret)), ("username", &self.username), ("name", &self.name)],
        )
    }

    pub fn new(request_id: RequestId, username: String, name: String, secret: &'static str) -> TestPrompt {
        TestPrompt { request_id, username, name, secret, text: Typed::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestLine {
    pub text: String,
    pub good: bool,
}

fn stage_name(stage: TestStage) -> String {
    match stage {
        TestStage::Settings => tr("Settings"),
        TestStage::Connect => tr("Connect"),
        TestStage::Login => tr("Login"),
        TestStage::StartFolder => tr("Start folder"),
    }
}

fn accept_first() -> String {
    tr("Connect once and accept it, then test again.")
}

pub fn describe(outcome: &TestOutcome) -> Option<TestLine> {
    let (text, good) = match outcome {
        TestOutcome::Connected(report) => {
            let mut text =
                i18n::text_with("gui.trial.connected_in", &[("ms", &report.elapsed.as_millis().to_string())]);
            if let Some(server) = &report.server {
                text.push_str(&format!(" \u{b7} {server}"));
            }
            let (sentence, good) = match &report.start_folder {
                StartFolder::Opens(path) => {
                    (Some(i18n::text_with("gui.trial.start_opens", &[("path", &path.to_string())])), true)
                }
                StartFolder::Missing(path) => {
                    (Some(i18n::text_with("gui.trial.start_missing", &[("path", &path.to_string())])), false)
                }
                StartFolder::Unreadable { path, message } => (
                    Some(i18n::text_with(
                        "gui.trial.start_unreadable",
                        &[("path", &path.to_string()), ("message", message)],
                    )),
                    false,
                ),
                StartFolder::Unchecked => (None, true),
            };
            if let Some(sentence) = sentence {
                text.push_str(". ");
                text.push_str(&sentence);
            }
            (text, good)
        }
        TestOutcome::NotTrusted(NotTrusted::HostKey { key_type, fingerprint }) => (
            i18n::text_with(
                "gui.trial.host_key",
                &[
                    ("key_type", &key_type.to_string()),
                    ("fingerprint", &fingerprint.to_string()),
                    ("advice", &accept_first()),
                ],
            ),
            false,
        ),
        TestOutcome::NotTrusted(NotTrusted::Certificate { fingerprint, subject }) => (
            i18n::text_with(
                "gui.trial.certificate",
                &[
                    ("subject", &subject.to_string()),
                    ("fingerprint", &fingerprint.to_string()),
                    ("advice", &accept_first()),
                ],
            ),
            false,
        ),
        TestOutcome::Failed { stage, message } => (
            i18n::text_with("gui.trial.failed", &[("stage", &stage_name(*stage)), ("message", &message.to_string())]),
            false,
        ),
        TestOutcome::TimedOut(stage) => {
            (i18n::text_with("gui.trial.timed_out_at", &[("stage", &stage_name(*stage))]), false)
        }
        TestOutcome::Cancelled => return None,
    };
    Some(TestLine { text, good })
}

#[cfg(test)]
mod tests;

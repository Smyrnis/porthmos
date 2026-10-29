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
    pub text: Typed,
}

impl TestPrompt {
    pub fn new(request_id: RequestId, username: String, name: String) -> TestPrompt {
        TestPrompt { request_id, username, name, text: Typed::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestLine {
    pub text: String,
    pub good: bool,
}

fn stage_name(stage: TestStage) -> &'static str {
    match stage {
        TestStage::Settings => "Settings",
        TestStage::Connect => "Connect",
        TestStage::Login => "Login",
        TestStage::StartFolder => "Start folder",
    }
}

const ACCEPT_FIRST: &str = "Connect once and accept it, then test again.";

pub fn describe(outcome: &TestOutcome) -> Option<TestLine> {
    let (text, good) = match outcome {
        TestOutcome::Connected(report) => {
            let mut text = format!("Connected in {}ms", report.elapsed.as_millis());
            if let Some(server) = &report.server {
                text.push_str(&format!(" \u{b7} {server}"));
            }
            let (sentence, good) = match &report.start_folder {
                StartFolder::Opens(path) => (Some(format!("Start folder {path} opens.")), true),
                StartFolder::Missing(path) => (Some(format!("Start folder {path} does not exist.")), false),
                StartFolder::Unreadable { path, message } => {
                    (Some(format!("Start folder {path} can't be read: {message}.")), false)
                }
                StartFolder::Unchecked => (None, true),
            };
            if let Some(sentence) = sentence {
                text.push_str(". ");
                text.push_str(&sentence);
            }
            (text, good)
        }
        TestOutcome::NotTrusted(NotTrusted::HostKey { key_type, fingerprint }) => {
            (format!("Host key not trusted yet ({key_type} {fingerprint}). {ACCEPT_FIRST}"), false)
        }
        TestOutcome::NotTrusted(NotTrusted::Certificate { fingerprint, subject }) => {
            (format!("Certificate not trusted yet ({subject}, {fingerprint}). {ACCEPT_FIRST}"), false)
        }
        TestOutcome::Failed { stage, message } => (format!("{} failed: {message}", stage_name(*stage)), false),
        TestOutcome::TimedOut(stage) => (format!("Timed out at {}", stage_name(*stage)), false),
        TestOutcome::Cancelled => return None,
    };
    Some(TestLine { text, good })
}

#[cfg(test)]
mod tests;

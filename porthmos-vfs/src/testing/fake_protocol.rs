use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;

use super::FakeFs;
use crate::{
    Answer, ConnectionForm, Environment, ErrorKind, FileSystem, Prompter, Protocol, ProtocolError, Question,
    ShellInvocation, Target,
};

pub struct FakeProtocol {
    id: &'static str,
    description: &'static str,
    limits_note: Option<&'static str>,
    form: Option<ConnectionForm>,
    fs: FakeFs,
    password: Option<String>,
    connect_failure: Option<(ErrorKind, String)>,
    connect_delay: Option<Duration>,
    host_key_question: Option<(String, String)>,
    certificate_question: Option<(String, String)>,
    login_delay: Option<Duration>,
    shell: Option<ShellInvocation>,
    discovered: Vec<Target>,
    seen: Arc<Mutex<Option<Target>>>,
}

impl FakeProtocol {
    pub fn new(fs: FakeFs) -> Self {
        Self {
            id: "fake",
            description: "",
            limits_note: None,
            form: None,
            fs,
            password: None,
            connect_failure: None,
            connect_delay: None,
            host_key_question: None,
            certificate_question: None,
            login_delay: None,
            shell: None,
            discovered: Vec::new(),
            seen: Arc::default(),
        }
    }

    pub fn requiring_password(mut self, password: &str) -> Self {
        self.password = Some(password.to_string());
        self
    }

    pub fn failing_connect(self, message: &str) -> Self {
        self.failing_with(ErrorKind::Connect, message)
    }

    pub fn failing_with(mut self, kind: ErrorKind, message: &str) -> Self {
        self.connect_failure = Some((kind, message.to_string()));
        self
    }

    pub fn with_connect_delay(mut self, delay: Duration) -> Self {
        self.connect_delay = Some(delay);
        self
    }

    pub fn with_login_delay(mut self, delay: Duration) -> Self {
        self.login_delay = Some(delay);
        self
    }

    pub fn asking_to_trust_host_key(mut self, key_type: &str, fingerprint: &str) -> Self {
        self.host_key_question = Some((key_type.to_string(), fingerprint.to_string()));
        self
    }

    pub fn asking_to_trust_certificate(mut self, fingerprint: &str, subject: &str) -> Self {
        self.certificate_question = Some((fingerprint.to_string(), subject.to_string()));
        self
    }

    pub fn with_id(mut self, id: &'static str) -> Self {
        self.id = id;
        self
    }

    pub fn with_description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub fn with_limits_note(mut self, note: &'static str) -> Self {
        self.limits_note = Some(note);
        self
    }

    pub fn with_form(mut self, form: ConnectionForm) -> Self {
        self.form = Some(form);
        self
    }

    pub fn with_shell(mut self, invocation: ShellInvocation) -> Self {
        self.shell = Some(invocation);
        self
    }

    pub fn seen_target(&self) -> Option<Target> {
        self.seen.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }

    pub fn seen_targets(&self) -> Arc<Mutex<Option<Target>>> {
        self.seen.clone()
    }

    pub fn with_discovered(mut self, targets: Vec<Target>) -> Self {
        self.discovered = targets;
        self
    }
}

#[async_trait]
impl Protocol for FakeProtocol {
    fn id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        "FAKE"
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn limits_note(&self) -> Option<&'static str> {
        self.limits_note
    }

    fn default_port(&self) -> u16 {
        2222
    }

    fn discover(&self, _env: &Environment) -> Result<Vec<Target>, ProtocolError> {
        Ok(self.discovered.clone())
    }

    fn connection_form(&self) -> ConnectionForm {
        self.form.clone().unwrap_or_else(|| ConnectionForm::standard(self.default_port()))
    }

    async fn connect(
        &self, target: &Target, prompter: &mut dyn Prompter,
    ) -> Result<Arc<dyn FileSystem>, ProtocolError> {
        *self.seen.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(target.clone());
        if let Some(delay) = self.connect_delay {
            tokio::time::sleep(delay).await;
        }
        if let Some((kind, message)) = &self.connect_failure {
            return Err(ProtocolError::new(*kind, anyhow::anyhow!(message.clone())));
        }
        if let Some((key_type, fingerprint)) = &self.host_key_question {
            let question = Question::TrustHostKey {
                name: target.name.clone(),
                host: target.host.clone(),
                port: target.port,
                key_type: key_type.clone(),
                fingerprint: fingerprint.clone(),
            };
            if !matches!(prompter.ask(question).await, Some(Answer::Confirmed)) {
                return Err(ProtocolError::new(ErrorKind::Cancelled, anyhow::anyhow!("Connection cancelled")));
            }
        }
        if let Some((fingerprint, subject)) = &self.certificate_question {
            let question = Question::TrustCertificate {
                name: target.name.clone(),
                host: target.host.clone(),
                port: target.port,
                fingerprint: fingerprint.clone(),
                subject: subject.clone(),
                expires: "2030-01-01".to_string(),
            };
            if !matches!(prompter.ask(question).await, Some(Answer::Confirmed)) {
                return Err(ProtocolError::new(ErrorKind::Cancelled, anyhow::anyhow!("Connection cancelled")));
            }
        }
        if let Some(expected) = &self.password
            && target.password.as_ref() != Some(expected)
        {
            let question = Question::Password { username: target.username.clone(), name: target.name.clone() };
            match prompter.ask(question).await {
                None => {
                    return Err(ProtocolError::new(ErrorKind::Cancelled, anyhow::anyhow!("Connection cancelled")));
                }
                Some(Answer::Password(given)) if &given == expected => {}
                Some(_) => {
                    return Err(ProtocolError::new(ErrorKind::AuthRejected, anyhow::anyhow!("rejected")));
                }
            }
        }
        if let Some(delay) = self.login_delay {
            tokio::time::sleep(delay).await;
        }
        Ok(Arc::new(self.fs.clone()))
    }

    fn shell_command(&self, _target: &Target, _env: &Environment) -> Option<ShellInvocation> {
        self.shell.clone()
    }
}

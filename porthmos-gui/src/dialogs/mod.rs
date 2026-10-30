use porthmos_core::{Question, RequestId};

#[derive(Clone, Default)]
pub struct Typed(String);

impl Typed {
    pub fn new(text: String) -> Typed {
        Typed(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for Typed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Typed(<redacted>)")
    }
}

#[derive(Debug, Clone)]
pub enum Dialog {
    Confirm { request_id: RequestId, title: String, body: String },
    Password { request_id: RequestId, title: String, prompt: String, text: Typed, save: Option<bool> },
}

impl Dialog {
    pub fn from_question(request_id: RequestId, question: Question, save: Option<bool>) -> Dialog {
        match question {
            Question::Password { username, name } => Dialog::Password {
                request_id,
                title: "Password".to_string(),
                prompt: format!("Password for {username} on {name}"),
                text: Typed::default(),
                save,
            },
            Question::TrustHostKey { name, host, port, key_type, fingerprint } => Dialog::Confirm {
                request_id,
                title: "Unknown host".to_string(),
                body: format!(
                    "{name} ({host}:{port}) is not a known host.\n{key_type} {fingerprint}\n\
                     Trust this key and add it to ~/.ssh/known_hosts?"
                ),
            },
            Question::TrustCertificate { name, host, port, fingerprint, subject, expires } => Dialog::Confirm {
                request_id,
                title: "Untrusted certificate".to_string(),
                body: format!(
                    "{name} ({host}:{port}) presented a certificate that isn't trusted.\n\
                     Subject: {subject}   Expires: {expires}\n{fingerprint}\nTrust it and remember it?"
                ),
            },
        }
    }

    pub fn request_id(&self) -> RequestId {
        match self {
            Dialog::Confirm { request_id, .. } | Dialog::Password { request_id, .. } => *request_id,
        }
    }
}

#[cfg(test)]
mod tests;

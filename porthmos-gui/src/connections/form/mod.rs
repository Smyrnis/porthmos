use std::collections::BTreeMap;
use std::fmt;

use porthmos_core::profiles::{ConnectionEntry, ProfileDraft, SecretEdit};
use porthmos_core::{ConnectionForm, OptionField, OptionKind, ProtocolInfo};

use super::trial::{TestPrompt, TestState};
use crate::dialogs::Typed;

const UNAVAILABLE_PORT: u16 = 22;
const PASSWORD: &str = "password";
const COMMON_KEYS: [&str; 9] =
    ["protocol", "name", "group", "tags", "host", "port", "username", "password", "remote_path"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    Text(String),
    Secret { saved: bool, forgotten: bool, typed: bool },
    Choice { choices: Vec<Choice>, selected: Choice },
    Toggle(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldView {
    pub key: String,
    pub label: String,
    pub required: bool,
    pub kind: FieldKind,
}

#[derive(Default)]
struct Secret {
    typed: Typed,
    forgotten: bool,
}

pub struct FormState {
    protocols: Vec<ProtocolInfo>,
    original: Option<ConnectionEntry>,
    protocol: String,
    values: BTreeMap<String, String>,
    secrets: BTreeMap<String, Secret>,
    port_untouched: bool,
    pub error: Option<String>,
    pub test: Option<TestState>,
    pub prompt: Option<TestPrompt>,
}

impl fmt::Debug for FormState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FormState")
            .field("protocol", &self.protocol)
            .field("original", &self.original.as_ref().map(|entry| &entry.name))
            .field("values", &self.values.keys().collect::<Vec<_>>())
            .field("secrets", &self.secrets.keys().collect::<Vec<_>>())
            .field("error", &self.error)
            .field("test", &self.test)
            .field("prompt", &self.prompt)
            .finish()
    }
}

fn form_in(protocols: &[ProtocolInfo], id: &str) -> ConnectionForm {
    protocols
        .iter()
        .find(|info| info.id == id)
        .map(|info| info.form.clone())
        .unwrap_or_else(|| ConnectionForm::standard(UNAVAILABLE_PORT))
}

fn default_text(field: &OptionField) -> Option<String> {
    match &field.kind {
        OptionKind::Text { default } => Some((*default).to_string()),
        OptionKind::Choice { default, .. } => Some((*default).to_string()),
        OptionKind::Toggle { default } => Some(default.to_string()),
        OptionKind::Secret => None,
    }
}

impl FormState {
    pub fn new(protocols: &[ProtocolInfo], original: Option<&ConnectionEntry>) -> Option<FormState> {
        let first = protocols.first()?;
        let protocol = original.map_or_else(|| first.id.to_string(), |entry| entry.protocol.clone());
        let form = form_in(protocols, &protocol);
        let mut values = BTreeMap::new();
        let mut port_untouched = true;
        match original {
            Some(entry) => {
                values.insert("name".to_string(), entry.name.clone());
                values.insert("group".to_string(), entry.group.clone().unwrap_or_default());
                values.insert("tags".to_string(), entry.tags.join(", "));
                values.insert("host".to_string(), entry.host.clone());
                values.insert("port".to_string(), entry.port.to_string());
                values.insert("username".to_string(), entry.username.clone());
                values.insert("remote_path".to_string(), entry.options.get("remote_path").cloned().unwrap_or_default());
                for field in &form.options {
                    if let Some(saved) = entry.options.get(field.key) {
                        values.insert(field.key.to_string(), saved.clone());
                    }
                }
                port_untouched = entry.port == form.port.default;
            }
            None => {
                for key in ["name", "group", "tags", "host", "username", "remote_path"] {
                    values.insert(key.to_string(), String::new());
                }
                values.insert("port".to_string(), form.port.default.to_string());
            }
        }
        let mut state = FormState {
            protocols: protocols.to_vec(),
            original: original.cloned(),
            protocol,
            values,
            secrets: BTreeMap::new(),
            port_untouched,
            error: None,
            test: None,
            prompt: None,
        };
        state.fill_defaults();
        Some(state)
    }

    fn form(&self) -> ConnectionForm {
        form_in(&self.protocols, &self.protocol)
    }

    fn fill_defaults(&mut self) {
        for field in self.form().options {
            if let Some(default) = default_text(&field) {
                self.values.entry(field.key.to_string()).or_insert(default);
            }
        }
    }

    pub fn title(&self) -> &'static str {
        if self.original.is_some() { "Edit connection" } else { "Add connection" }
    }

    #[cfg(test)]
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original.as_ref().map(|entry| entry.name.as_str())
    }

    pub fn protocol_choices(&self) -> Vec<Choice> {
        let mut choices: Vec<Choice> = self
            .protocols
            .iter()
            .map(|info| Choice { value: info.id.to_string(), label: info.display_name.to_string() })
            .collect();
        if !self.protocols.iter().any(|info| info.id == self.protocol) {
            choices.push(Choice { value: self.protocol.clone(), label: format!("{} (not available)", self.protocol) });
        }
        choices
    }

    pub fn set_protocol(&mut self, id: &str) {
        if self.protocol == id {
            return;
        }
        let typed_port = self.values.get("port").map(|port| port.trim().to_string()).unwrap_or_default();
        self.protocol = id.to_string();
        if self.port_untouched || typed_port.is_empty() {
            let default = self.form().port.default;
            self.values.insert("port".to_string(), default.to_string());
            self.port_untouched = true;
        }
        self.fill_defaults();
        self.error = None;
    }

    pub fn set_text(&mut self, key: &str, value: String) {
        if key == "protocol" {
            self.set_protocol(&value);
            return;
        }
        if key == "port" {
            self.port_untouched = false;
        }
        self.values.insert(key.to_string(), value);
        self.error = None;
    }

    pub fn set_choice(&mut self, key: &str, value: &str) {
        self.set_text(key, value.to_string());
    }

    pub fn set_toggle(&mut self, key: &str, on: bool) {
        self.set_text(key, on.to_string());
    }

    pub fn set_secret(&mut self, key: &str, value: String) {
        let secret = self.secrets.entry(key.to_string()).or_default();
        secret.typed = Typed::new(value);
        secret.forgotten = false;
        self.error = None;
    }

    pub fn forget_secret(&mut self, key: &str) {
        let secret = self.secrets.entry(key.to_string()).or_default();
        secret.typed = Typed::default();
        secret.forgotten = true;
    }

    pub fn undo_forget_secret(&mut self, key: &str) {
        if let Some(secret) = self.secrets.get_mut(key) {
            secret.forgotten = false;
        }
    }

    pub fn secret_text(&self, key: &str) -> &str {
        self.secrets.get(key).map_or("", |secret| secret.typed.as_str())
    }

    fn saved(&self, key: &str) -> bool {
        self.original
            .as_ref()
            .is_some_and(|entry| entry.protocol == self.protocol && entry.in_keyring.iter().any(|saved| saved == key))
    }

    fn secret_edit(&self, key: &str) -> SecretEdit {
        let typed = self.secret_text(key);
        if !typed.is_empty() {
            SecretEdit::Replace(typed.to_string())
        } else if self.secrets.get(key).is_some_and(|secret| secret.forgotten) || !self.saved(key) {
            SecretEdit::Clear
        } else {
            SecretEdit::Keep
        }
    }

    fn secret_view(&self, key: &str, label: &str, required: bool) -> FieldView {
        let forgotten = self.secrets.get(key).is_some_and(|secret| secret.forgotten);
        let kind = FieldKind::Secret { saved: self.saved(key), forgotten, typed: !self.secret_text(key).is_empty() };
        FieldView { key: key.to_string(), label: label.to_string(), required, kind }
    }

    fn text_view(&self, key: &str, label: &str, required: bool) -> FieldView {
        let value = self.values.get(key).cloned().unwrap_or_default();
        FieldView { key: key.to_string(), label: label.to_string(), required, kind: FieldKind::Text(value) }
    }

    pub fn fields(&self) -> Vec<FieldView> {
        let form = self.form();
        let choices = self.protocol_choices();
        let selected = choices.iter().find(|choice| choice.value == self.protocol).cloned().unwrap();
        let mut fields = vec![
            FieldView {
                key: "protocol".to_string(),
                label: "Protocol".to_string(),
                required: false,
                kind: FieldKind::Choice { choices, selected },
            },
            self.text_view("name", "Name", true),
            self.text_view("group", "Group", false),
            self.text_view("tags", "Tags", false),
            self.text_view("host", form.host.label, form.host.required),
            self.text_view("port", form.port.label, false),
            self.text_view("username", form.username.label, form.username.required),
            self.secret_view(PASSWORD, form.password.label, form.password.required),
            self.text_view("remote_path", "Remote folder", false),
        ];
        for field in &form.options {
            fields.push(self.option_view(field));
        }
        fields
    }

    fn option_view(&self, field: &OptionField) -> FieldView {
        let value = self.values.get(field.key).cloned().unwrap_or_default();
        match &field.kind {
            OptionKind::Secret => self.secret_view(field.key, field.label, field.required),
            OptionKind::Text { .. } => self.text_view(field.key, field.label, field.required),
            OptionKind::Toggle { .. } => FieldView {
                key: field.key.to_string(),
                label: field.label.to_string(),
                required: field.required,
                kind: FieldKind::Toggle(value == "true"),
            },
            OptionKind::Choice { choices, default } => {
                let all: Vec<Choice> = choices
                    .iter()
                    .map(|choice| Choice { value: choice.value.to_string(), label: choice.label.to_string() })
                    .collect();
                let selected = all
                    .iter()
                    .find(|choice| choice.value == value)
                    .or_else(|| all.iter().find(|choice| choice.value == *default))
                    .cloned()
                    .unwrap_or_else(|| Choice { value: value.clone(), label: value });
                FieldView {
                    key: field.key.to_string(),
                    label: field.label.to_string(),
                    required: field.required,
                    kind: FieldKind::Choice { choices: all, selected },
                }
            }
        }
    }

    pub fn check(&self) -> Result<(), String> {
        self.draft().validate(&self.form(), self.original.as_ref()).map(|_| ())
    }

    pub fn draft(&self) -> ProfileDraft {
        let form = self.form();
        let text = |key: &str| self.values.get(key).cloned().unwrap_or_default();
        let mut draft = ProfileDraft {
            protocol: self.protocol.clone(),
            name: text("name"),
            host: text("host"),
            port: text("port"),
            username: text("username"),
            password: self.secret_edit(PASSWORD),
            remote_path: text("remote_path"),
            group: text("group"),
            tags: text("tags"),
            ..ProfileDraft::default()
        };
        for field in &form.options {
            debug_assert!(!COMMON_KEYS.contains(&field.key));
            if field.kind == OptionKind::Secret {
                draft.secret_options.insert(field.key.to_string(), self.secret_edit(field.key));
            } else {
                draft.options.insert(field.key.to_string(), text(field.key));
            }
        }
        draft
    }
}

#[cfg(test)]
mod tests;

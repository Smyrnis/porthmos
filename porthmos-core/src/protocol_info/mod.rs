use porthmos_vfs::{ConnectionForm, Protocol};

use crate::i18n;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub limits_note: Option<&'static str>,
    pub form: ConnectionForm,
}

impl ProtocolInfo {
    pub fn from_protocol(protocol: &dyn Protocol) -> Self {
        Self {
            id: protocol.id(),
            display_name: protocol.display_name(),
            description: protocol.description(),
            limits_note: protocol.limits_note(),
            form: protocol.connection_form(),
        }
    }

    pub fn description_text(&self) -> String {
        i18n::text_or(&format!("protocol.{}.description", self.id), self.description)
    }

    pub fn limits_note_text(&self) -> Option<String> {
        self.limits_note.map(|note| i18n::text_or(&format!("protocol.{}.limits_note", self.id), note))
    }
}

#[cfg(test)]
mod tests;

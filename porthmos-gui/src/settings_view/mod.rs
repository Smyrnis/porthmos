use porthmos_core::UpdateMode;

use crate::shortcuts::Action;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSettings {
    pub editor: Option<String>,
    pub auto_upload: bool,
    pub update_mode: UpdateMode,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self { editor: None, auto_upload: true, update_mode: UpdateMode::Auto }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsMessage {
    Error(String),
    Note(String),
}

#[derive(Debug)]
pub struct SettingsState {
    pub capturing: Option<Action>,
    pub editor_text: String,
    pub message: Option<SettingsMessage>,
}

impl SettingsState {
    pub fn new(settings: &GuiSettings) -> Self {
        Self { capturing: None, editor_text: settings.editor.clone().unwrap_or_default(), message: None }
    }
}

pub fn editor_from_text(text: &str) -> Option<String> {
    Some(text.trim()).filter(|editor| !editor.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests;

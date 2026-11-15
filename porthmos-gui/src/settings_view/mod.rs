use porthmos_core::theme::ThemeBase;
use porthmos_core::{ReopenMode, UpdateMode};

use crate::shortcuts::Action;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSettings {
    pub editor: Option<String>,
    pub terminal: Option<String>,
    pub auto_upload: bool,
    pub compare_limit_mib: u64,
    pub update_mode: UpdateMode,
    pub search_preview: bool,
    pub reopen: ReopenMode,
    pub reconnect: bool,
    pub theme_base: ThemeBase,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            editor: None,
            terminal: None,
            auto_upload: true,
            compare_limit_mib: porthmos_core::config::settings::DEFAULT_COMPARE_LIMIT_MIB,
            update_mode: UpdateMode::Auto,
            search_preview: false,
            reopen: ReopenMode::Off,
            reconnect: false,
            theme_base: ThemeBase::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Shortcuts,
    Appearance,
    Editing,
    Terminal,
    Search,
    Startup,
    Connections,
    Updates,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsCategory {
    #[default]
    General,
    EditorAndTerminal,
    Appearance,
    Shortcuts,
    Updates,
}

impl SettingsCategory {
    pub const ALL: [SettingsCategory; 5] = [
        SettingsCategory::General,
        SettingsCategory::EditorAndTerminal,
        SettingsCategory::Appearance,
        SettingsCategory::Shortcuts,
        SettingsCategory::Updates,
    ];

    pub fn title(self) -> &'static str {
        match self {
            SettingsCategory::General => "General",
            SettingsCategory::EditorAndTerminal => "Editor and terminal",
            SettingsCategory::Appearance => "Appearance",
            SettingsCategory::Shortcuts => "Shortcuts",
            SettingsCategory::Updates => "Updates",
        }
    }

    pub fn sections(self) -> &'static [SettingsSection] {
        match self {
            SettingsCategory::General => {
                &[SettingsSection::Startup, SettingsSection::Connections, SettingsSection::Search]
            }
            SettingsCategory::EditorAndTerminal => &[SettingsSection::Editing, SettingsSection::Terminal],
            SettingsCategory::Appearance => &[SettingsSection::Appearance],
            SettingsCategory::Shortcuts => &[SettingsSection::Shortcuts],
            SettingsCategory::Updates => &[SettingsSection::Updates],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsMessage {
    Error(String),
    Note(String),
}

#[derive(Debug)]
pub struct SettingsState {
    pub category: SettingsCategory,
    pub capturing: Option<Action>,
    pub editor_text: String,
    pub compare_limit_text: String,
    pub terminal_text: String,
    pub message: Option<SettingsMessage>,
}

impl SettingsState {
    pub fn new(settings: &GuiSettings) -> Self {
        Self {
            category: SettingsCategory::default(),
            capturing: None,
            editor_text: settings.editor.clone().unwrap_or_default(),
            compare_limit_text: settings.compare_limit_mib.to_string(),
            terminal_text: settings.terminal.clone().unwrap_or_default(),
            message: None,
        }
    }
}

pub fn compare_limit_from_text(text: &str) -> Option<u64> {
    use porthmos_core::config::settings::{MAX_COMPARE_LIMIT_MIB, MIN_COMPARE_LIMIT_MIB};
    text.trim().parse::<u64>().ok().filter(|mib| (MIN_COMPARE_LIMIT_MIB..=MAX_COMPARE_LIMIT_MIB).contains(mib))
}

pub fn editor_from_text(text: &str) -> Option<String> {
    Some(text.trim()).filter(|editor| !editor.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests;

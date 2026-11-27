use porthmos_core::i18n;
use porthmos_core::i18n::label as tr;
use porthmos_core::theme::ThemeBase;
use porthmos_core::{LogsSettings, RefreshSettings, ReopenMode, UpdateMode};

use crate::shortcuts::Action;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiSettings {
    pub editor: Option<String>,
    pub terminal: Option<String>,
    pub embedded_terminal: bool,
    pub auto_upload: bool,
    pub compare_limit_mib: u64,
    pub update_mode: UpdateMode,
    pub search_preview: bool,
    pub search_max_depth: usize,
    pub search_max_results: usize,
    pub reopen: ReopenMode,
    pub reconnect: bool,
    pub refresh: RefreshSettings,
    pub logs: LogsSettings,
    pub theme_base: ThemeBase,
    pub language: Option<String>,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            editor: None,
            terminal: None,
            embedded_terminal: true,
            auto_upload: true,
            compare_limit_mib: porthmos_core::config::settings::DEFAULT_COMPARE_LIMIT_MIB,
            update_mode: UpdateMode::Auto,
            search_preview: false,
            search_max_depth: porthmos_core::config::settings::DEFAULT_SEARCH_DEPTH,
            search_max_results: porthmos_core::config::settings::DEFAULT_SEARCH_RESULTS,
            reopen: ReopenMode::Off,
            reconnect: false,
            refresh: RefreshSettings::default(),
            logs: LogsSettings::default(),
            theme_base: ThemeBase::Dark,
            language: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Language,
    Shortcuts,
    Appearance,
    Editing,
    Terminal,
    Search,
    Startup,
    Connections,
    Updates,
    Refresh,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogsField {
    MaxLines,
    InitialLines,
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

    pub fn title(self) -> String {
        match self {
            SettingsCategory::General => tr("General"),
            SettingsCategory::EditorAndTerminal => tr("Editor and terminal"),
            SettingsCategory::Appearance => tr("Appearance"),
            SettingsCategory::Shortcuts => tr("Shortcuts"),
            SettingsCategory::Updates => tr("Updates"),
        }
    }

    pub fn sections(self) -> &'static [SettingsSection] {
        match self {
            SettingsCategory::General => &[
                SettingsSection::Language,
                SettingsSection::Startup,
                SettingsSection::Connections,
                SettingsSection::Search,
                SettingsSection::Refresh,
                SettingsSection::Logs,
            ],
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
    pub search_depth_text: String,
    pub search_results_text: String,
    pub terminal_text: String,
    pub refresh_text: [String; 3],
    pub logs_max_text: String,
    pub logs_initial_text: String,
    pub message: Option<SettingsMessage>,
}

impl SettingsState {
    pub fn new(settings: &GuiSettings) -> Self {
        Self {
            category: SettingsCategory::default(),
            capturing: None,
            editor_text: settings.editor.clone().unwrap_or_default(),
            compare_limit_text: settings.compare_limit_mib.to_string(),
            search_depth_text: settings.search_max_depth.to_string(),
            search_results_text: settings.search_max_results.to_string(),
            terminal_text: settings.terminal.clone().unwrap_or_default(),
            refresh_text: crate::refresh::ALL
                .map(|tab| crate::refresh::setting(&settings.refresh, tab).interval_seconds.to_string()),
            logs_max_text: settings.logs.max_lines.to_string(),
            logs_initial_text: settings.logs.initial_lines.to_string(),
            message: None,
        }
    }
}

pub fn compare_limit_from_text(text: &str) -> Option<u64> {
    use porthmos_core::config::settings::{MAX_COMPARE_LIMIT_MIB, MIN_COMPARE_LIMIT_MIB};
    text.trim().parse::<u64>().ok().filter(|mib| (MIN_COMPARE_LIMIT_MIB..=MAX_COMPARE_LIMIT_MIB).contains(mib))
}

pub fn refresh_seconds_from_text(text: &str) -> Option<u64> {
    use porthmos_core::config::settings::{MAX_REFRESH_SECONDS, MIN_REFRESH_SECONDS};
    text.trim().parse::<u64>().ok().filter(|seconds| (MIN_REFRESH_SECONDS..=MAX_REFRESH_SECONDS).contains(seconds))
}

pub fn logs_max_lines_from_text(text: &str) -> Option<u32> {
    use porthmos_core::config::settings::LOGS_MAX_LINES_RANGE;
    text.trim().parse::<u32>().ok().filter(|lines| LOGS_MAX_LINES_RANGE.contains(lines))
}

pub fn logs_initial_lines_from_text(text: &str) -> Option<u32> {
    use porthmos_core::config::settings::LOGS_INITIAL_LINES_RANGE;
    text.trim().parse::<u32>().ok().filter(|lines| LOGS_INITIAL_LINES_RANGE.contains(lines))
}

pub fn logs_bounds(field: LogsField) -> (u32, u32) {
    use porthmos_core::config::settings::{LOGS_INITIAL_LINES_RANGE, LOGS_MAX_LINES_RANGE};
    let range = match field {
        LogsField::MaxLines => LOGS_MAX_LINES_RANGE,
        LogsField::InitialLines => LOGS_INITIAL_LINES_RANGE,
    };
    (*range.start(), *range.end())
}

pub fn logs_range_text(field: LogsField) -> String {
    let (min, max) = logs_bounds(field);
    i18n::text_with("gui.settings.logs_range", &[("min", &min.to_string()), ("max", &max.to_string())])
}

pub fn search_depth_from_text(text: &str) -> Option<usize> {
    use porthmos_core::config::settings::{MAX_SEARCH_DEPTH, MIN_SEARCH_DEPTH};
    text.trim().parse::<usize>().ok().filter(|depth| (MIN_SEARCH_DEPTH..=MAX_SEARCH_DEPTH).contains(depth))
}

pub fn search_results_from_text(text: &str) -> Option<usize> {
    use porthmos_core::config::settings::{MAX_SEARCH_RESULTS, MIN_SEARCH_RESULTS};
    text.trim().parse::<usize>().ok().filter(|count| (MIN_SEARCH_RESULTS..=MAX_SEARCH_RESULTS).contains(count))
}

pub fn editor_from_text(text: &str) -> Option<String> {
    Some(text.trim()).filter(|editor| !editor.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests;

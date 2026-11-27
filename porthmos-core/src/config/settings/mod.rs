use serde::{Deserialize, Serialize};

use crate::{
    listing::{SortKey, SortOrder, SortSpec},
    transfer::conflicts::ConflictPolicy,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanelSettings {
    pub show_hidden: bool,
    pub sort: SortSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSettings {
    pub auto_upload: bool,
    pub editor: Option<String>,
    pub compare_limit_mib: u64,
}

pub const DEFAULT_COMPARE_LIMIT_MIB: u64 = 1;
pub const MIN_COMPARE_LIMIT_MIB: u64 = 1;
pub const MAX_COMPARE_LIMIT_MIB: u64 = 64;
const BYTES_PER_MIB: u64 = 1024 * 1024;

impl Default for EditSettings {
    fn default() -> Self {
        Self { auto_upload: true, editor: None, compare_limit_mib: DEFAULT_COMPARE_LIMIT_MIB }
    }
}

impl EditSettings {
    pub fn compare_limit_bytes(&self) -> u64 {
        self.compare_limit_mib * BYTES_PER_MIB
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSettings {
    pub command: Option<String>,
    pub embedded: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self { command: None, embedded: true }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UpdateMode {
    #[default]
    Auto,
    Notify,
    Off,
}

pub const MIN_UPDATE_INTERVAL_HOURS: u64 = 1;
pub const MAX_UPDATE_INTERVAL_HOURS: u64 = 720;
pub const DEFAULT_UPDATE_INTERVAL_HOURS: u64 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateSettings {
    pub mode: UpdateMode,
    pub interval_hours: u64,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { mode: UpdateMode::Auto, interval_hours: DEFAULT_UPDATE_INTERVAL_HOURS }
    }
}

pub const DEFAULT_SEARCH_DEPTH: usize = 16;
pub const MIN_SEARCH_DEPTH: usize = 1;
pub const MAX_SEARCH_DEPTH: usize = 64;
pub const DEFAULT_SEARCH_RESULTS: usize = 1000;
pub const MIN_SEARCH_RESULTS: usize = 10;
pub const MAX_SEARCH_RESULTS: usize = 10000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchSettings {
    pub preview: bool,
    pub max_depth: usize,
    pub max_results: usize,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self { preview: false, max_depth: DEFAULT_SEARCH_DEPTH, max_results: DEFAULT_SEARCH_RESULTS }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReopenMode {
    #[default]
    Off,
    Ask,
    Auto,
}

impl ReopenMode {
    pub fn id(self) -> &'static str {
        match self {
            ReopenMode::Off => "off",
            ReopenMode::Ask => "ask",
            ReopenMode::Auto => "auto",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConnectionSettings {
    pub reconnect: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartupSettings {
    pub reopen: ReopenMode,
}

pub const MAX_PARALLEL_CAP: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferSettings {
    pub max_parallel: usize,
    pub on_conflict: ConflictPolicy,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self { max_parallel: 4, on_conflict: ConflictPolicy::Ask }
    }
}

pub const DEFAULT_REFRESH_SECONDS: u64 = 10;
pub const MIN_REFRESH_SECONDS: u64 = 5;
pub const MAX_REFRESH_SECONDS: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshSetting {
    pub auto: bool,
    pub interval_seconds: u64,
}

impl Default for RefreshSetting {
    fn default() -> Self {
        Self { auto: false, interval_seconds: DEFAULT_REFRESH_SECONDS }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefreshSettings {
    pub health: RefreshSetting,
    pub services: RefreshSetting,
    pub ports: RefreshSetting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTab {
    Health,
    Services,
    Ports,
}

impl RefreshTab {
    pub fn name(self) -> &'static str {
        match self {
            RefreshTab::Health => "health",
            RefreshTab::Services => "services",
            RefreshTab::Ports => "ports",
        }
    }
}

pub const LOGS_MAX_LINES_RANGE: std::ops::RangeInclusive<u32> = 1000..=200000;
pub const LOGS_INITIAL_LINES_RANGE: std::ops::RangeInclusive<u32> = 0..=5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogsSettings {
    pub max_lines: u32,
    pub initial_lines: u32,
}

impl Default for LogsSettings {
    fn default() -> Self {
        Self { max_lines: 20000, initial_lines: 500 }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub panel: PanelSettings,
    pub transfers: TransferSettings,
    pub edit: EditSettings,
    pub terminal: TerminalSettings,
    pub update: UpdateSettings,
    pub search: SearchSettings,
    pub startup: StartupSettings,
    pub connection: ConnectionSettings,
    pub refresh: RefreshSettings,
    pub logs: LogsSettings,
    pub language: Option<String>,
    pub frontend: toml::Table,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct SettingsFile {
    #[serde(default)]
    pub panel: PanelSettingsFile,
    #[serde(default)]
    pub transfers: TransferSettingsFile,
    #[serde(default)]
    pub edit: EditSettingsFile,
    #[serde(default)]
    pub terminal: TerminalSettingsFile,
    #[serde(default)]
    pub update: UpdateSettingsFile,
    #[serde(default)]
    pub search: SearchSettingsFile,
    #[serde(default)]
    pub startup: StartupSettingsFile,
    #[serde(default)]
    pub connection: ConnectionSettingsFile,
    #[serde(default)]
    pub refresh: RefreshSettingsFile,
    #[serde(default)]
    pub logs: LogsSettingsFile,
    #[serde(default)]
    pub general: GeneralSettingsFile,
    #[serde(flatten)]
    pub frontend: toml::Table,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct GeneralSettingsFile {
    pub language: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct TransferSettingsFile {
    pub max_parallel: Option<i64>,
    pub on_conflict: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct SearchSettingsFile {
    pub preview: Option<bool>,
    pub max_depth: Option<i64>,
    pub max_results: Option<i64>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct ConnectionSettingsFile {
    pub reconnect: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct RefreshSettingsFile {
    #[serde(default)]
    pub health: RefreshGroupFile,
    #[serde(default)]
    pub services: RefreshGroupFile,
    #[serde(default)]
    pub ports: RefreshGroupFile,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct RefreshGroupFile {
    pub auto: Option<toml::Value>,
    pub interval_seconds: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct LogsSettingsFile {
    pub max_lines: Option<toml::Value>,
    pub initial_lines: Option<toml::Value>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct StartupSettingsFile {
    pub reopen: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct UpdateSettingsFile {
    pub mode: Option<String>,
    pub interval_hours: Option<i64>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct EditSettingsFile {
    pub auto_upload: Option<bool>,
    pub editor: Option<String>,
    pub compare_limit_mib: Option<i64>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct TerminalSettingsFile {
    pub command: Option<String>,
    pub embedded: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct PanelSettingsFile {
    pub show_hidden: Option<bool>,
    pub sort_key: Option<String>,
    pub sort_order: Option<String>,
}

fn refresh_from_file(tab: RefreshTab, file: RefreshGroupFile, warnings: &mut Vec<String>) -> RefreshSetting {
    let name = tab.name();
    let default = RefreshSetting::default();
    let auto = match file.auto {
        None => default.auto,
        Some(toml::Value::Boolean(auto)) => auto,
        Some(_) => {
            warnings.push(format!("refresh.{name}.auto must be true or false, using {}", default.auto));
            default.auto
        }
    };
    let interval_seconds = match file.interval_seconds {
        None => default.interval_seconds,
        Some(toml::Value::Integer(seconds))
            if (MIN_REFRESH_SECONDS as i64..=MAX_REFRESH_SECONDS as i64).contains(&seconds) =>
        {
            seconds as u64
        }
        Some(toml::Value::Integer(_)) => {
            warnings.push(format!(
                "refresh.{name}.interval_seconds must be between {MIN_REFRESH_SECONDS} and {MAX_REFRESH_SECONDS}, using {DEFAULT_REFRESH_SECONDS}"
            ));
            default.interval_seconds
        }
        Some(_) => {
            warnings.push(format!(
                "refresh.{name}.interval_seconds must be a whole number, using {DEFAULT_REFRESH_SECONDS}"
            ));
            default.interval_seconds
        }
    };
    RefreshSetting { auto, interval_seconds }
}

fn logs_value(
    key: &str, value: Option<toml::Value>, range: &std::ops::RangeInclusive<u32>, default: u32,
    warnings: &mut Vec<String>,
) -> u32 {
    match value {
        None => default,
        Some(toml::Value::Integer(count)) if u32::try_from(count).is_ok_and(|count| range.contains(&count)) => {
            count as u32
        }
        Some(toml::Value::Integer(_)) => {
            warnings.push(format!("logs.{key} must be between {} and {}, using {default}", range.start(), range.end()));
            default
        }
        Some(_) => {
            warnings.push(format!("logs.{key} must be a whole number, using {default}"));
            default
        }
    }
}

pub(crate) fn settings_from_file(file: SettingsFile) -> (Settings, Vec<String>) {
    let mut warnings = Vec::new();
    let show_hidden = file.panel.show_hidden.unwrap_or(false);

    let key = match file.panel.sort_key.as_deref() {
        None => SortKey::Name,
        Some(text) => SortKey::parse(text).unwrap_or_else(|| {
            warnings.push(format!("unknown panel.sort_key \"{text}\", using \"name\""));
            SortKey::Name
        }),
    };

    let order = match file.panel.sort_order.as_deref() {
        None | Some("ascending") => SortOrder::Ascending,
        Some("descending") => SortOrder::Descending,
        Some(other) => {
            warnings.push(format!("unknown panel.sort_order \"{other}\", using \"ascending\""));
            SortOrder::Ascending
        }
    };

    let max_parallel = match file.transfers.max_parallel {
        Some(value) if value > MAX_PARALLEL_CAP as i64 => {
            warnings.push(format!("transfers.max_parallel is capped at {MAX_PARALLEL_CAP}, using {MAX_PARALLEL_CAP}"));
            MAX_PARALLEL_CAP
        }
        Some(value) if value >= 1 => value as usize,
        Some(_) => {
            warnings.push("transfers.max_parallel must be at least 1, using 1".to_string());
            1
        }
        None => TransferSettings::default().max_parallel,
    };

    let on_conflict = match file.transfers.on_conflict.as_deref() {
        None | Some("ask") => ConflictPolicy::Ask,
        Some("overwrite") => ConflictPolicy::Overwrite,
        Some("skip") => ConflictPolicy::Skip,
        Some("rename") => ConflictPolicy::Rename,
        Some(other) => {
            warnings.push(format!("unknown transfers.on_conflict \"{other}\", using \"ask\""));
            ConflictPolicy::Ask
        }
    };

    let update_mode = match file.update.mode.as_deref() {
        None | Some("auto") => UpdateMode::Auto,
        Some("notify") => UpdateMode::Notify,
        Some("off") => UpdateMode::Off,
        Some(other) => {
            warnings.push(format!("unknown update.mode \"{other}\", using \"auto\""));
            UpdateMode::Auto
        }
    };

    let update_interval = match file.update.interval_hours {
        None => DEFAULT_UPDATE_INTERVAL_HOURS,
        Some(hours) if (MIN_UPDATE_INTERVAL_HOURS as i64..=MAX_UPDATE_INTERVAL_HOURS as i64).contains(&hours) => {
            hours as u64
        }
        Some(_) => {
            warnings.push(format!(
                "update.interval_hours must be between {MIN_UPDATE_INTERVAL_HOURS} and {MAX_UPDATE_INTERVAL_HOURS}, using {DEFAULT_UPDATE_INTERVAL_HOURS}"
            ));
            DEFAULT_UPDATE_INTERVAL_HOURS
        }
    };

    let compare_limit_mib = match file.edit.compare_limit_mib {
        None => DEFAULT_COMPARE_LIMIT_MIB,
        Some(mib) if (MIN_COMPARE_LIMIT_MIB as i64..=MAX_COMPARE_LIMIT_MIB as i64).contains(&mib) => mib as u64,
        Some(_) => {
            warnings.push(format!(
                "edit.compare_limit_mib must be between {MIN_COMPARE_LIMIT_MIB} and {MAX_COMPARE_LIMIT_MIB}, using {DEFAULT_COMPARE_LIMIT_MIB}"
            ));
            DEFAULT_COMPARE_LIMIT_MIB
        }
    };

    let search_depth = match file.search.max_depth {
        None => DEFAULT_SEARCH_DEPTH,
        Some(depth) if (MIN_SEARCH_DEPTH as i64..=MAX_SEARCH_DEPTH as i64).contains(&depth) => depth as usize,
        Some(_) => {
            warnings.push(format!(
                "search.max_depth must be between {MIN_SEARCH_DEPTH} and {MAX_SEARCH_DEPTH}, using {DEFAULT_SEARCH_DEPTH}"
            ));
            DEFAULT_SEARCH_DEPTH
        }
    };

    let search_results = match file.search.max_results {
        None => DEFAULT_SEARCH_RESULTS,
        Some(count) if (MIN_SEARCH_RESULTS as i64..=MAX_SEARCH_RESULTS as i64).contains(&count) => count as usize,
        Some(_) => {
            warnings.push(format!(
                "search.max_results must be between {MIN_SEARCH_RESULTS} and {MAX_SEARCH_RESULTS}, using {DEFAULT_SEARCH_RESULTS}"
            ));
            DEFAULT_SEARCH_RESULTS
        }
    };

    let reopen = match file.startup.reopen.as_deref() {
        None | Some("off") => ReopenMode::Off,
        Some("ask") => ReopenMode::Ask,
        Some("auto") => ReopenMode::Auto,
        Some(other) => {
            warnings.push(format!("unknown startup.reopen \"{other}\", using \"off\""));
            ReopenMode::Off
        }
    };

    let language = match file.general.language.as_deref() {
        None => None,
        Some(setting) if crate::i18n::is_automatic(setting) => None,
        Some(setting) if crate::i18n::is_available(setting.trim()) => Some(setting.trim().to_string()),
        Some(other) => {
            warnings.push(format!("unknown general.language \"{other}\", using the language of the system"));
            None
        }
    };

    let refresh = RefreshSettings {
        health: refresh_from_file(RefreshTab::Health, file.refresh.health, &mut warnings),
        services: refresh_from_file(RefreshTab::Services, file.refresh.services, &mut warnings),
        ports: refresh_from_file(RefreshTab::Ports, file.refresh.ports, &mut warnings),
    };

    let logs_default = LogsSettings::default();
    let logs = LogsSettings {
        max_lines: logs_value(
            "max_lines",
            file.logs.max_lines,
            &LOGS_MAX_LINES_RANGE,
            logs_default.max_lines,
            &mut warnings,
        ),
        initial_lines: logs_value(
            "initial_lines",
            file.logs.initial_lines,
            &LOGS_INITIAL_LINES_RANGE,
            logs_default.initial_lines,
            &mut warnings,
        ),
    };

    (
        Settings {
            panel: PanelSettings { show_hidden, sort: SortSpec { key, order } },
            transfers: TransferSettings { max_parallel, on_conflict },
            edit: EditSettings {
                auto_upload: file.edit.auto_upload.unwrap_or(true),
                editor: file.edit.editor.map(|editor| editor.trim().to_string()).filter(|editor| !editor.is_empty()),
                compare_limit_mib,
            },
            terminal: TerminalSettings {
                command: file
                    .terminal
                    .command
                    .map(|command| command.trim().to_string())
                    .filter(|command| !command.is_empty()),
                embedded: file.terminal.embedded.unwrap_or(true),
            },
            update: UpdateSettings { mode: update_mode, interval_hours: update_interval },
            search: SearchSettings {
                preview: file.search.preview.unwrap_or(false),
                max_depth: search_depth,
                max_results: search_results,
            },
            startup: StartupSettings { reopen },
            connection: ConnectionSettings { reconnect: file.connection.reconnect.unwrap_or(false) },
            refresh,
            logs,
            language,
            frontend: file.frontend,
        },
        warnings,
    )
}

#[cfg(test)]
mod tests;

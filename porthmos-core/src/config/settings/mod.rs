use serde::{Deserialize, Serialize};

use crate::{
    listing::{SortKey, SortOrder, SortSpec},
    transfer::conflicts::ConflictPolicy,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanelSettings {
    pub show_hidden: bool,
    pub sort: SortSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSettings {
    pub auto_upload: bool,
    pub editor: Option<String>,
}

impl Default for EditSettings {
    fn default() -> Self {
        Self { auto_upload: true, editor: None }
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub panel: PanelSettings,
    pub transfers: TransferSettings,
    pub edit: EditSettings,
    pub update: UpdateSettings,
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
    pub update: UpdateSettingsFile,
    #[serde(flatten)]
    pub frontend: toml::Table,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct TransferSettingsFile {
    pub max_parallel: Option<i64>,
    pub on_conflict: Option<String>,
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
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct PanelSettingsFile {
    pub show_hidden: Option<bool>,
    pub sort_key: Option<String>,
    pub sort_order: Option<String>,
}

pub(crate) fn settings_from_file(file: SettingsFile) -> (Settings, Vec<String>) {
    let mut warnings = Vec::new();
    let show_hidden = file.panel.show_hidden.unwrap_or(false);

    let key = match file.panel.sort_key.as_deref() {
        None | Some("name") => SortKey::Name,
        Some("size") => SortKey::Size,
        Some(other) => {
            warnings.push(format!("unknown panel.sort_key \"{other}\", using \"name\""));
            SortKey::Name
        }
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

    (
        Settings {
            panel: PanelSettings { show_hidden, sort: SortSpec { key, order } },
            transfers: TransferSettings { max_parallel, on_conflict },
            edit: EditSettings {
                auto_upload: file.edit.auto_upload.unwrap_or(true),
                editor: file.edit.editor.map(|editor| editor.trim().to_string()).filter(|editor| !editor.is_empty()),
            },
            update: UpdateSettings { mode: update_mode, interval_hours: update_interval },
            frontend: file.frontend,
        },
        warnings,
    )
}

#[cfg(test)]
mod tests;

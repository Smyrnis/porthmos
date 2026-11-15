use serde::{Deserialize, Serialize};

use crate::{
    config::settings::PanelSettings,
    listing::{SortKey, SortOrder, SortSpec},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelMemory {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<String>,
}

impl PanelMemory {
    pub fn of(settings: &PanelSettings) -> Self {
        let key = match settings.sort.key {
            SortKey::Name => "name",
            SortKey::Size => "size",
        };
        let order = match settings.sort.order {
            SortOrder::Ascending => "ascending",
            SortOrder::Descending => "descending",
        };
        Self {
            show_hidden: Some(settings.show_hidden),
            sort_key: Some(key.to_string()),
            sort_order: Some(order.to_string()),
        }
    }

    pub fn applied_to(&self, base: &PanelSettings) -> PanelSettings {
        let key = match self.sort_key.as_deref() {
            Some("name") => SortKey::Name,
            Some("size") => SortKey::Size,
            _ => base.sort.key,
        };
        let order = match self.sort_order.as_deref() {
            Some("ascending") => SortOrder::Ascending,
            Some("descending") => SortOrder::Descending,
            _ => base.sort.order,
        };
        PanelSettings { show_hidden: self.show_hidden.unwrap_or(base.show_hidden), sort: SortSpec { key, order } }
    }
}

#[cfg(test)]
mod tests;

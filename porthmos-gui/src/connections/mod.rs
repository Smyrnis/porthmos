pub mod form;
pub mod trial;

use std::collections::HashSet;

use porthmos_core::connections_tree::{self, TreeRow};
use porthmos_core::profiles::{ConnectionEntry, ConnectionSource};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cursor {
    Entry { name: String, source: ConnectionSource },
    Group(String),
}

#[derive(Debug, Default)]
pub struct Manager {
    entries: Vec<ConnectionEntry>,
    filter: String,
    expanded: HashSet<String>,
    cursor: Option<Cursor>,
}

impl Manager {
    pub fn replace(&mut self, entries: Vec<ConnectionEntry>) {
        self.entries = entries;
        let gone = match &self.cursor {
            Some(Cursor::Entry { name, source }) => {
                !self.entries.iter().any(|entry| entry.name == *name && entry.source == *source)
            }
            _ => false,
        };
        if gone {
            self.cursor = None;
        }
        self.drop_hidden_selection();
    }

    pub fn entries(&self) -> &[ConnectionEntry] {
        &self.entries
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn set_filter(&mut self, text: String) {
        self.filter = text;
        self.drop_hidden_selection();
    }

    fn drop_hidden_selection(&mut self) {
        if self.cursor.is_some() && self.selected_row().is_none() {
            self.cursor = None;
        }
    }

    fn active_filter(&self) -> Option<&str> {
        connections_tree::active_filter(Some(&self.filter))
    }

    pub fn match_count(&self) -> usize {
        connections_tree::match_count(&self.entries, self.active_filter())
    }

    pub fn rows(&self) -> Vec<TreeRow> {
        connections_tree::rows(&self.entries, &self.expanded, self.active_filter())
    }

    pub fn toggle_group(&mut self, path: &str) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_string());
        }
        self.drop_hidden_selection();
    }

    fn cursor_of(&self, row: &TreeRow) -> Cursor {
        match row {
            TreeRow::Group { path, .. } => Cursor::Group(path.clone()),
            TreeRow::Connection { index, .. } => {
                let entry = &self.entries[*index];
                Cursor::Entry { name: entry.name.clone(), source: entry.source }
            }
        }
    }

    pub fn select_row(&mut self, row: usize) {
        if let Some(found) = self.rows().get(row) {
            self.cursor = Some(self.cursor_of(found));
        }
    }

    pub fn selected_row(&self) -> Option<usize> {
        let cursor = self.cursor.as_ref()?;
        self.rows().iter().position(|row| &self.cursor_of(row) == cursor)
    }

    pub fn selected_entry(&self) -> Option<&ConnectionEntry> {
        match self.cursor.as_ref()? {
            Cursor::Entry { name, source } => {
                self.entries.iter().find(|entry| entry.name == *name && entry.source == *source)
            }
            Cursor::Group(_) => None,
        }
    }

    pub fn move_selection(&mut self, down: bool) {
        let rows = self.rows();
        if rows.is_empty() {
            return;
        }
        let target = match (self.selected_row(), down) {
            (None, true) => 0,
            (None, false) => rows.len() - 1,
            (Some(current), true) => (current + 1).min(rows.len() - 1),
            (Some(current), false) => current.saturating_sub(1),
        };
        self.cursor = Some(self.cursor_of(&rows[target]));
    }

    pub fn unlabelled_hosts(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.source == ConnectionSource::SshConfig)
            .filter(|entry| entry.group.is_none() && entry.tags.is_empty())
            .map(|entry| entry.name.clone())
            .collect()
    }

    pub fn reveal(&mut self, name: &str) {
        let found = self
            .entries
            .iter()
            .find(|entry| entry.name == name && !entry.source.is_orphan_labels())
            .or_else(|| self.entries.iter().find(|entry| entry.name == name));
        let Some(entry) = found else {
            return;
        };
        let hidden = connections_tree::active_filter(Some(&self.filter))
            .is_some_and(|filter| !connections_tree::matches(entry, filter));
        let cursor = Cursor::Entry { name: entry.name.clone(), source: entry.source };
        let group = entry.group.clone();
        if hidden {
            self.filter.clear();
        }
        self.cursor = Some(cursor);
        if let Some(group) = group {
            let mut path = String::new();
            for segment in group.split('/') {
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(segment);
                self.expanded.insert(path.clone());
            }
        }
    }
}

#[derive(Debug)]
pub enum Screen {
    List,
    Form(Box<form::FormState>),
    ConfirmDelete { name: String },
    Labels(LabelsState),
    MissingHost { name: String, shadowed: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelsState {
    pub name: String,
    pub group: String,
    pub tags: String,
    pub saved_password: bool,
    pub forget_password: bool,
    pub error: Option<String>,
}

impl LabelsState {
    pub fn for_entry(entry: &ConnectionEntry) -> LabelsState {
        LabelsState {
            name: entry.name.clone(),
            group: entry.group.clone().unwrap_or_default(),
            tags: entry.tags.join(", "),
            saved_password: entry.saved_password,
            forget_password: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingChoice {
    Move(String),
    Forget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagerButtons {
    pub connect: bool,
    pub edit: bool,
    pub delete: bool,
}

impl Manager {
    pub fn buttons(&self) -> ManagerButtons {
        match self.selected_entry().map(|entry| entry.source) {
            Some(ConnectionSource::Profile) => ManagerButtons { connect: true, edit: true, delete: true },
            Some(ConnectionSource::SshConfig) => ManagerButtons { connect: true, edit: true, delete: false },
            Some(ConnectionSource::MissingSshHost | ConnectionSource::ShadowedSshHost) => {
                ManagerButtons { connect: false, edit: true, delete: false }
            }
            None => ManagerButtons { connect: false, edit: false, delete: false },
        }
    }
}

pub fn details_lines(entry: &ConnectionEntry, protocol_name: &str, connected: bool) -> Vec<(String, String)> {
    let mut lines = vec![
        ("Protocol".to_string(), protocol_name.to_string()),
        ("Host".to_string(), entry.host.clone()),
        ("Port".to_string(), entry.port.to_string()),
        ("User".to_string(), entry.username.clone()),
        ("Group".to_string(), entry.group.clone().unwrap_or_default()),
        ("Tags".to_string(), entry.tags.join(", ")),
        ("Saved password".to_string(), if entry.saved_password { "yes" } else { "" }.to_string()),
        ("Start folder".to_string(), entry.options.get("remote_path").cloned().unwrap_or_default()),
    ];
    lines.retain(|(_, value)| !value.is_empty());
    let status = if connected { "Connected" } else { "Not connected" };
    lines.push(("Status".to_string(), status.to_string()));
    lines
}

pub fn announcement(entries: &[ConnectionEntry], announced: &mut HashSet<String>) -> Option<String> {
    let unannounced = |source: ConnectionSource| -> Vec<String> {
        let mut names: Vec<String> = entries
            .iter()
            .filter(|entry| entry.source == source && !announced.contains(&entry.name))
            .map(|entry| entry.name.clone())
            .collect();
        names.sort();
        names
    };
    let missing = unannounced(ConnectionSource::MissingSshHost);
    let shadowed = unannounced(ConnectionSource::ShadowedSshHost);
    if missing.is_empty() && shadowed.is_empty() {
        return None;
    }
    let mut sentences = Vec::new();
    if !missing.is_empty() {
        sentences.push(format!("These labelled hosts are no longer in ~/.ssh/config: {}.", missing.join(", ")));
    }
    if !shadowed.is_empty() {
        sentences.push(format!(
            "These labelled hosts are hidden by a saved connection of the same name: {}.",
            shadowed.join(", ")
        ));
    }
    announced.extend(missing.into_iter().chain(shadowed));
    Some(sentences.join(" "))
}

pub fn badge(entry: &ConnectionEntry) -> String {
    entry.protocol.to_uppercase()
}

pub fn subtitle(entry: &ConnectionEntry) -> String {
    if entry.source.is_orphan_labels() {
        return "labels only".to_string();
    }
    format!("{}@{}:{}", entry.username, entry.host, entry.port)
}

#[cfg(test)]
mod tests;

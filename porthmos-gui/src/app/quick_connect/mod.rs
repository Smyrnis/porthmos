use std::cmp::Reverse;

use porthmos_core::profiles::ConnectionSource;

use super::App;

pub const QUICK_CONNECTION_LIMIT: usize = 5;

impl App {
    fn ranked_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .manager
            .entries()
            .iter()
            .filter(|entry| matches!(entry.source, ConnectionSource::Profile | ConnectionSource::SshConfig))
            .map(|entry| entry.name.as_str())
            .collect();
        names.sort_by_key(|name| Reverse(self.last_used.get(*name)));
        names
    }

    pub fn quick_connections(&self) -> Vec<String> {
        self.ranked_names().into_iter().take(QUICK_CONNECTION_LIMIT).map(str::to_string).collect()
    }

    pub fn quick_connections_hidden(&self) -> usize {
        self.ranked_names().len().saturating_sub(QUICK_CONNECTION_LIMIT)
    }

    pub(in crate::app) fn quick_connect_pressed(&mut self, name: &str) {
        self.connect_profile(name);
    }
}

#[cfg(test)]
mod tests;

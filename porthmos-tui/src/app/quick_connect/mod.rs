use std::cmp::Reverse;

use porthmos_core::profiles::ConnectionSource;

use super::{App, ConnectionStatus};

pub const QUICK_CONNECTION_LIMIT: usize = 5;

impl App {
    fn ranked_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .connections
            .entries()
            .iter()
            .filter(|entry| matches!(entry.source, ConnectionSource::Profile | ConnectionSource::SshConfig))
            .map(|entry| entry.name.as_str())
            .collect();
        names.sort_by_key(|name| Reverse(self.last_used.get(*name)));
        names
    }

    pub(super) fn quick_connections(&self) -> Vec<String> {
        self.ranked_names().into_iter().take(QUICK_CONNECTION_LIMIT).map(str::to_string).collect()
    }

    pub(super) fn quick_connections_hidden(&self) -> usize {
        self.ranked_names().len().saturating_sub(QUICK_CONNECTION_LIMIT)
    }

    pub(super) fn quick_connect(&mut self, number: usize) -> bool {
        if matches!(self.connection_status, ConnectionStatus::Connecting(_)) {
            return false;
        }
        let Some(name) = number.checked_sub(1).and_then(|index| self.quick_connections().into_iter().nth(index)) else {
            return false;
        };
        self.connect_profile(&name)
    }
}

#[cfg(test)]
mod tests;

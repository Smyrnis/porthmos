use std::cmp::Reverse;

use porthmos_core::{Location, SessionId};

use crate::shortcuts::Action;

pub mod matcher;

use matcher::fuzzy_match;

pub const MAX_HITS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Commands,
    Hosts,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Command(Action),
    ConnectionManager,
    Connect(String),
    Disconnect(SessionId),
    File { location: Location, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub group: Group,
    pub label: String,
    pub hint: Option<String>,
    pub target: Target,
}

#[derive(Debug)]
pub struct PaletteState {
    entries: Vec<Entry>,
    query: String,
    hits: Vec<usize>,
    total: usize,
    selected: usize,
}

fn scope_of(query: &str) -> (&'static [Group], &str) {
    let groups: &'static [Group] = match query.chars().next() {
        Some('>') => &[Group::Commands],
        Some('@') => &[Group::Hosts],
        Some('#') => &[Group::Files],
        _ => return (&[Group::Commands, Group::Hosts], query),
    };
    (groups, query[1..].trim_start_matches(' '))
}

impl PaletteState {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut state = Self { entries, query: String::new(), hits: Vec::new(), total: 0, selected: 0 };
        state.refresh();
        state
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.refresh();
    }

    pub fn replace_entries(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
        self.refresh();
    }

    pub fn hits(&self) -> Vec<&Entry> {
        self.hits.iter().map(|index| &self.entries[*index]).collect()
    }

    pub fn hit_count(&self) -> usize {
        self.hits.len()
    }

    pub fn total_matches(&self) -> usize {
        self.total
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, row: usize) {
        if row < self.hits.len() {
            self.selected = row;
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.hits.is_empty() {
            return;
        }
        let last = self.hits.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.hits.get(self.selected).map(|index| &self.entries[*index])
    }

    fn refresh(&mut self) {
        let (groups, text) = scope_of(&self.query);
        let mut scored: Vec<(Group, i32, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| groups.contains(&entry.group))
            .filter_map(|(index, entry)| Some((entry.group, fuzzy_match(text, &entry.label)?.score, index)))
            .collect();
        scored.sort_by_key(|(group, score, _)| (*group, Reverse(*score)));
        self.total = scored.len();
        self.hits = scored.into_iter().take(MAX_HITS).map(|(_, _, index)| index).collect();
        self.selected = 0;
    }
}

#[cfg(test)]
mod tests;

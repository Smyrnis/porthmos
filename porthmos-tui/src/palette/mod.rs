use std::cmp::Reverse;

use porthmos_core::Location;

use crate::{input::Action, widgets::history_view::printable};

pub mod matcher;

use matcher::fuzzy_match;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Actions,
    Hosts,
    Files,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Actions => "actions",
            Group::Hosts => "hosts",
            Group::Files => "files",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Action(Action),
    Connect(String),
    Disconnect(String),
    File { location: Location, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub group: Group,
    pub label: String,
    pub hint: Option<String>,
    pub target: Target,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub entry: usize,
    pub matched: Vec<usize>,
}

pub struct Palette {
    entries: Vec<Entry>,
    query: String,
    caret: usize,
    hits: Vec<Hit>,
    selected: usize,
}

fn scope_of(query: &str) -> (&'static [Group], &str) {
    let groups: &'static [Group] = match query.chars().next() {
        Some('>') => &[Group::Actions],
        Some('@') => &[Group::Hosts],
        Some('#') => &[Group::Files],
        _ => return (&[Group::Actions, Group::Hosts], query),
    };
    (groups, query[1..].trim_start_matches(' '))
}

impl Palette {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut palette = Self { entries, query: String::new(), caret: 0, hits: Vec::new(), selected: 0 };
        palette.refresh();
        palette
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    pub fn entry(&self, hit: &Hit) -> &Entry {
        &self.entries[hit.entry]
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.hits.get(self.selected).map(|hit| self.entry(hit))
    }

    pub fn type_char(&mut self, character: char) {
        let text = character.to_string();
        if printable(&text) != text {
            return;
        }
        let at = self.byte_index(self.caret);
        self.query.insert(at, character);
        self.caret += 1;
        self.refresh();
    }

    pub fn backspace(&mut self) {
        if self.caret == 0 {
            return;
        }
        let at = self.byte_index(self.caret - 1);
        self.query.remove(at);
        self.caret -= 1;
        self.refresh();
    }

    pub fn delete(&mut self) {
        if self.caret >= self.query.chars().count() {
            return;
        }
        let at = self.byte_index(self.caret);
        self.query.remove(at);
        self.refresh();
    }

    pub fn move_caret(&mut self, delta: isize) {
        let length = self.query.chars().count() as isize;
        self.caret = (self.caret as isize + delta).clamp(0, length) as usize;
    }

    pub fn caret_home(&mut self) {
        self.caret = 0;
    }

    pub fn caret_end(&mut self) {
        self.caret = self.query.chars().count();
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.hits.is_empty() {
            return;
        }
        let last = self.hits.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }

    fn byte_index(&self, characters: usize) -> usize {
        self.query.char_indices().nth(characters).map_or(self.query.len(), |(byte, _)| byte)
    }

    fn refresh(&mut self) {
        let (groups, text) = scope_of(&self.query);
        let mut scored: Vec<(Group, i32, Hit)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| groups.contains(&entry.group))
            .filter_map(|(index, entry)| {
                let found = fuzzy_match(text, &entry.label)?;
                Some((entry.group, found.score, Hit { entry: index, matched: found.indices }))
            })
            .collect();
        scored.sort_by_key(|(group, score, _)| (*group, Reverse(*score)));
        self.hits = scored.into_iter().map(|(_, _, hit)| hit).collect();
        self.selected = 0;
    }
}

#[cfg(test)]
mod tests;

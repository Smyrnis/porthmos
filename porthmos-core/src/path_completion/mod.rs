use porthmos_vfs::Entry;

use crate::Location;

pub const MAX_SHOWN: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Completion {
    pub prefix: String,
    pub candidates: Vec<String>,
    pub completed: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionStep {
    Replace(String),
    Show(Vec<String>),
    Nothing,
}

fn is_separator(character: char, location: Location) -> bool {
    character == '/' || (cfg!(windows) && location == Location::Local && character == '\\')
}

pub fn split_directory(text: &str, location: Location) -> (&str, &str) {
    match text.rfind(|character| is_separator(character, location)) {
        Some(index) => text.split_at(index + 1),
        None => ("", text),
    }
}

fn separator_after(prefix: &str) -> char {
    prefix.chars().next_back().unwrap_or('/')
}

fn shared_prefix(names: &[String]) -> String {
    let Some(first) = names.first() else {
        return String::new();
    };
    let mut shared: Vec<char> = first.chars().collect();
    for name in &names[1..] {
        let common = shared.iter().zip(name.chars()).take_while(|(left, right)| **left == *right).count();
        shared.truncate(common);
    }
    shared.into_iter().collect()
}

pub fn complete(text: &str, location: Location, folder_entries: &[Entry]) -> Completion {
    if text == "~" {
        return Completion { prefix: String::new(), candidates: Vec::new(), completed: "~/".to_string() };
    }
    let (prefix, fragment) = split_directory(text, location);
    let wants_hidden = fragment.starts_with('.');
    let mut candidates: Vec<String> = folder_entries
        .iter()
        .filter(|entry| {
            entry.is_dir && (wants_hidden || !entry.name.starts_with('.')) && entry.name.starts_with(fragment)
        })
        .map(|entry| entry.name.clone())
        .collect();
    candidates.sort();
    let mut completed = format!("{prefix}{}", shared_prefix(&candidates));
    if candidates.len() == 1 {
        completed.push(separator_after(prefix));
    }
    if candidates.is_empty() {
        completed = text.to_string();
    }
    Completion { prefix: prefix.to_string(), candidates, completed }
}

impl Completion {
    pub fn step(&self, typed: &str) -> CompletionStep {
        if self.completed != typed {
            CompletionStep::Replace(self.completed.clone())
        } else if self.candidates.len() > 1 {
            CompletionStep::Show(self.candidates.clone())
        } else {
            CompletionStep::Nothing
        }
    }

    pub fn shown(&self) -> (&[String], usize) {
        let count = self.candidates.len().min(MAX_SHOWN);
        (&self.candidates[..count], self.candidates.len() - count)
    }

    pub fn path_for(&self, name: &str) -> String {
        format!("{}{name}{}", self.prefix, separator_after(&self.prefix))
    }
}

#[cfg(test)]
mod tests;

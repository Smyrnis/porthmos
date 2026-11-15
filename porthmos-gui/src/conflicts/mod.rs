use std::collections::VecDeque;

use porthmos_core::Command;
use porthmos_core::transfer::conflicts::{ConflictInfo, Resolution, fits_the_rest};
use porthmos_core::transfer::plan::ExistingFile;

const SECONDS_SHOWN: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictView {
    pub file_name: String,
    pub existing: Option<ExistingFile>,
    pub partial: Option<ExistingFile>,
    pub new_size: u64,
    pub new_modified: Option<u64>,
    pub index: usize,
    pub total: usize,
}

impl ConflictView {
    pub fn can_resume(&self) -> bool {
        self.partial.is_some()
    }

    pub fn freshness(&self) -> Option<&'static str> {
        let (new, old) = (self.new_modified? / SECONDS_SHOWN, self.existing?.modified? / SECONDS_SHOWN);
        match new.cmp(&old) {
            std::cmp::Ordering::Greater => Some("newer"),
            std::cmp::Ordering::Less => Some("older"),
            std::cmp::Ordering::Equal => None,
        }
    }
}

#[derive(Debug)]
struct Prompt {
    batch_id: u64,
    files: Vec<ConflictInfo>,
    answers: Vec<Option<Resolution>>,
    cursor: usize,
}

impl Prompt {
    fn view(&self) -> ConflictView {
        let file = &self.files[self.cursor];
        ConflictView {
            file_name: file.display_name.clone(),
            existing: file.existing,
            partial: file.partial,
            new_size: file.new_size,
            new_modified: file.new_modified,
            index: self.cursor,
            total: self.files.len(),
        }
    }

    fn advance(&mut self) -> bool {
        while self.cursor < self.answers.len() && self.answers[self.cursor].is_some() {
            self.cursor += 1;
        }
        self.cursor < self.answers.len()
    }
}

#[derive(Debug, Default)]
pub struct ConflictQueue {
    prompts: VecDeque<Prompt>,
}

impl ConflictQueue {
    pub fn found(&mut self, batch_id: u64, files: Vec<ConflictInfo>) {
        if files.is_empty() {
            return;
        }
        let answers = vec![None; files.len()];
        self.prompts.push_back(Prompt { batch_id, files, answers, cursor: 0 });
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }

    pub fn current(&self) -> Option<ConflictView> {
        self.prompts.front().map(Prompt::view)
    }

    pub fn answer(&mut self, resolution: Option<Resolution>, apply_to_rest: bool) -> Option<Command> {
        let prompt = self.prompts.front_mut()?;
        let Some(resolution) = resolution else {
            let batch_id = prompt.batch_id;
            self.prompts.pop_front();
            return Some(Command::ResolveConflicts { batch_id, answers: None });
        };
        let current = prompt.cursor;
        if resolution == Resolution::Resume && prompt.files[current].partial.is_none() {
            return None;
        }
        prompt.answers[current] = Some(resolution);
        if apply_to_rest {
            let (answered, later) = prompt.files.split_at(current + 1);
            let answered = &answered[current];
            for (offset, file) in later.iter().enumerate() {
                let slot = &mut prompt.answers[current + 1 + offset];
                if slot.is_none() && fits_the_rest(resolution, answered, file) {
                    *slot = Some(resolution);
                }
            }
        }
        if prompt.advance() {
            return None;
        }
        let prompt = self.prompts.pop_front()?;
        let answers = prompt.answers.into_iter().collect::<Option<Vec<Resolution>>>();
        Some(Command::ResolveConflicts { batch_id: prompt.batch_id, answers })
    }

    pub fn withdraw(&mut self, batch_ids: &[u64]) -> bool {
        let showing = self.prompts.front().is_some_and(|prompt| batch_ids.contains(&prompt.batch_id));
        self.prompts.retain(|prompt| !batch_ids.contains(&prompt.batch_id));
        showing
    }
}

#[cfg(test)]
mod tests;

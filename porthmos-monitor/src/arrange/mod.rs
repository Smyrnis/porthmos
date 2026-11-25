use std::cmp::Ordering;

use crate::{MAX_PROCESSES, Process};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Cpu,
    Memory,
}

fn key(value: Option<f32>) -> f32 {
    value.filter(|value| value.is_finite()).unwrap_or(f32::NEG_INFINITY)
}

fn order(sort: Sort, left: &Process, right: &Process) -> Ordering {
    let (left_key, right_key) = match sort {
        Sort::Cpu => (key(left.cpu_percent), key(right.cpu_percent)),
        Sort::Memory => (key(Some(left.memory_percent)), key(Some(right.memory_percent))),
    };
    right_key.total_cmp(&left_key).then(left.pid.cmp(&right.pid))
}

fn matches(process: &Process, needle: &str) -> bool {
    needle.is_empty()
        || [Some(process.command.as_str()), Some(process.name.as_str()), process.user.as_deref()]
            .into_iter()
            .flatten()
            .any(|text| text.to_lowercase().contains(needle))
}

pub fn arranged<'a>(all: &'a [Process], sort: Sort, filter: &str, selected: Option<u32>) -> Vec<&'a Process> {
    let needle = filter.trim().to_lowercase();
    let mut shown: Vec<&Process> = all.iter().filter(|process| matches(process, &needle)).collect();
    shown.sort_by(|left, right| order(sort, left, right));
    let kept_selected = shown.iter().skip(MAX_PROCESSES).copied().find(|process| Some(process.pid) == selected);
    shown.truncate(MAX_PROCESSES);
    shown.extend(kept_selected);
    shown
}

#[cfg(test)]
mod tests;

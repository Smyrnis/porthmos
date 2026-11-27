use std::collections::{HashMap, HashSet};

use super::{
    MAX_UNITS,
    unit::{ActiveState, BootState, Unit},
};
use crate::parse::sections;

const NAME_LIMIT: usize = 256;
const DESCRIPTION_LIMIT: usize = 256;
const STATE_LIMIT: usize = 64;
const TEMPLATE_SUFFIX: &str = "@.service";

fn next_field(rest: &str) -> Option<(&str, &str)> {
    let rest = rest.trim_start();
    if rest.is_empty() {
        return None;
    }
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(rest.split_at(end))
}

fn without_marker(line: &str) -> &str {
    let line = line.trim_start();
    match next_field(line) {
        Some((first, rest)) if first.chars().count() == 1 && !first.chars().all(char::is_alphanumeric) => rest,
        _ => line,
    }
}

fn clean_name(name: &str) -> Option<String> {
    sections::text(name.as_bytes(), NAME_LIMIT).filter(|cleaned| cleaned == name)
}

fn state(text: &str) -> String {
    sections::text(text.as_bytes(), STATE_LIMIT).unwrap_or_default()
}

fn listed(line: &[u8]) -> Option<Unit> {
    let text = String::from_utf8_lossy(line);
    let rest = without_marker(&text);
    let (name, rest) = next_field(rest)?;
    let (load, rest) = next_field(rest)?;
    let (active, rest) = next_field(rest)?;
    let (sub, rest) = next_field(rest)?;
    Some(Unit {
        name: clean_name(name)?,
        description: sections::text(rest.as_bytes(), DESCRIPTION_LIMIT).unwrap_or_default(),
        load: state(load),
        active: ActiveState::from_text(active),
        sub: state(sub),
        boot: None,
    })
}

fn boot_states(lines: &[&[u8]]) -> Vec<(String, BootState)> {
    lines
        .iter()
        .filter_map(|line| {
            let text = String::from_utf8_lossy(line);
            let (name, rest) = next_field(without_marker(&text))?;
            let (boot, _) = next_field(rest)?;
            Some((clean_name(name)?, BootState::from_text(boot)))
        })
        .collect()
}

pub(super) fn units(listed_lines: &[&[u8]], file_lines: &[&[u8]]) -> Vec<Unit> {
    let mut boot: HashMap<String, BootState> = HashMap::new();
    let files = boot_states(file_lines);
    for (name, state) in &files {
        boot.entry(name.clone()).or_insert(*state);
    }
    let mut seen = HashSet::new();
    let mut units: Vec<Unit> = listed_lines
        .iter()
        .filter_map(|line| listed(line))
        .filter(|unit| seen.insert(unit.name.clone()))
        .take(MAX_UNITS)
        .map(|mut unit| {
            unit.boot = boot.get(&unit.name).copied();
            unit
        })
        .collect();
    let cut_off = listed_lines.len() > MAX_UNITS;
    for (name, state) in files {
        if cut_off || units.len() >= MAX_UNITS {
            break;
        }
        if name.ends_with(TEMPLATE_SUFFIX) || !seen.insert(name.clone()) {
            continue;
        }
        let load = if state == BootState::Masked { "masked" } else { "loaded" };
        units.push(Unit {
            name,
            description: String::new(),
            load: load.to_string(),
            active: ActiveState::Inactive,
            sub: "dead".to_string(),
            boot: Some(state),
        });
    }
    units
}

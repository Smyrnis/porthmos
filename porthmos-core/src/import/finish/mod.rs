use std::{collections::HashSet, sync::Arc};

use porthmos_vfs::{OptionKind, Protocol};

use super::{Candidate, ImportRow, Verdict};

const REMOTE_PATH: &str = "remote_path";
const MAX_NAME: usize = 100;
const MAX_HOST: usize = 253;
const MAX_USERNAME: usize = 128;
const MAX_GROUP: usize = 200;
const MAX_PATH: usize = 1024;
const FIRST_SUFFIX: u32 = 2;

fn invalid(row: &mut ImportRow, reason: String) {
    row.verdict = Verdict::Invalid(reason);
    row.candidate = None;
}

fn natural_port(protocol: &str, security: Option<&str>) -> Option<u16> {
    match (protocol, security) {
        ("ftp", Some("implicit")) => Some(990),
        ("webdav", Some("http")) => Some(80),
        ("webdav", Some("https")) => Some(443),
        _ => None,
    }
}

fn longest_field_over_its_limit(candidate: &Candidate) -> Option<&'static str> {
    let over = |text: &str, limit: usize| text.chars().count() > limit;
    if over(&candidate.name, MAX_NAME) {
        Some("name")
    } else if over(&candidate.host, MAX_HOST) {
        Some("host")
    } else if over(&candidate.username, MAX_USERNAME) {
        Some("username")
    } else if candidate.group.as_deref().is_some_and(|group| over(group, MAX_GROUP)) {
        Some("group")
    } else if candidate.options.get(REMOTE_PATH).is_some_and(|path| over(path, MAX_PATH)) {
        Some("remote_path")
    } else {
        None
    }
}

fn checked(mut row: ImportRow, protocols: &[Arc<dyn Protocol>]) -> ImportRow {
    if row.verdict != Verdict::New {
        return row;
    }
    let Some(mut candidate) = row.candidate.take() else {
        return row;
    };
    let Some(protocol) = protocols.iter().find(|protocol| protocol.id() == candidate.protocol) else {
        row.verdict =
            Verdict::Unsupported(format!("{} is not available in this build", candidate.protocol.to_uppercase()));
        return row;
    };
    if let Some(field) = longest_field_over_its_limit(&candidate) {
        invalid(&mut row, format!("{field} is too long"));
        return row;
    }
    let form = protocol.connection_form();
    if form.username.required && candidate.username.is_empty() {
        invalid(&mut row, "no username".to_string());
        return row;
    }
    candidate
        .options
        .retain(|key, _| key == REMOTE_PATH || form.option(key).is_some_and(|field| field.kind != OptionKind::Secret));
    for (key, value) in &candidate.options {
        let Some(field) = form.option(key) else {
            continue;
        };
        let problem = match &field.kind {
            OptionKind::Choice { choices, .. } if !choices.iter().any(|choice| choice.value == value) => {
                Some(format!("{} must be one of the offered choices", field.label))
            }
            OptionKind::Toggle { .. } if value != "true" && value != "false" => {
                Some(format!("{} must be on or off", field.label))
            }
            _ => None,
        };
        if let Some(problem) = problem {
            invalid(&mut row, problem);
            return row;
        }
    }
    if candidate.port.is_none() {
        candidate.port = natural_port(&candidate.protocol, candidate.options.get("security").map(String::as_str));
    }
    row.candidate = Some(candidate);
    row
}

fn make_names_unique(rows: &mut [ImportRow]) {
    let originals: HashSet<String> =
        rows.iter().filter(|row| row.verdict == Verdict::New).map(|row| row.name.clone()).collect();
    let mut seen: HashSet<String> = HashSet::new();
    for row in rows.iter_mut().filter(|row| row.verdict == Verdict::New) {
        if seen.insert(row.name.clone()) {
            continue;
        }
        let base = row.name.clone();
        let renamed = (FIRST_SUFFIX..)
            .map(|number| format!("{base} ({number})"))
            .find(|name| !originals.contains(name) && !seen.contains(name))
            .unwrap_or_else(|| base.clone());
        seen.insert(renamed.clone());
        if let Some(candidate) = row.candidate.as_mut() {
            candidate.name = renamed.clone();
        }
        row.name = renamed;
    }
}

pub(crate) fn finish(
    rows: Vec<ImportRow>, protocols: &[Arc<dyn Protocol>], taken_names: &HashSet<String>,
) -> Vec<ImportRow> {
    let mut rows: Vec<ImportRow> = rows.into_iter().map(|row| checked(row, protocols)).collect();
    make_names_unique(&mut rows);
    for row in rows.iter_mut().filter(|row| row.verdict == Verdict::New) {
        if taken_names.contains(&row.name) {
            row.verdict = Verdict::Exists;
        }
    }
    rows
}

#[cfg(test)]
mod tests;

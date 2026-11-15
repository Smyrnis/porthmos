use std::{collections::HashSet, sync::Arc};

use porthmos_vfs::{OptionKind, Protocol};

use super::{ImportRow, Verdict};

const REMOTE_PATH: &str = "remote_path";
const FIRST_SUFFIX: u32 = 2;

fn invalid(row: &mut ImportRow, reason: String) {
    row.verdict = Verdict::Invalid(reason);
    row.candidate = None;
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

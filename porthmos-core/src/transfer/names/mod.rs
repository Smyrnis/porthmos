use std::collections::{HashMap, HashSet};

use porthmos_vfs::{NameRules, VfsPath};

use super::{conflicts::unique_name, plan::PlannedFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renamed {
    pub from: String,
    pub to: String,
}

#[derive(Default)]
pub(crate) struct Renames {
    seen: HashSet<VfsPath>,
    list: Vec<Renamed>,
    spellings: HashMap<String, String>,
}

impl Renames {
    pub(crate) fn sanitize_under(&mut self, root: &VfsPath, path: &VfsPath, rules: NameRules) -> VfsPath {
        self.clean(root, path, rules, false)
    }

    pub(crate) fn sanitize_folder(&mut self, root: &VfsPath, path: &VfsPath, rules: NameRules) -> VfsPath {
        self.clean(root, path, rules, true)
    }

    fn clean(&mut self, root: &VfsPath, path: &VfsPath, rules: NameRules, spell_last: bool) -> VfsPath {
        let Some(relative) = path.strip_prefix(root) else {
            return path.clone();
        };
        let count = relative.segments().count();
        let mut original = root.clone();
        let mut clean = root.clone();
        for (index, segment) in relative.segments().enumerate() {
            let name = segment.to_string_lossy();
            let mut cleaned = rules.sanitize(&name);
            original = original.join(name.as_ref());
            if cleaned != name && self.seen.insert(original.clone()) {
                self.list.push(Renamed { from: name.into_owned(), to: cleaned.clone() });
            }
            if !rules.is_case_sensitive() && (spell_last || index + 1 < count) {
                let key = rules.fold(&clean.join(&cleaned).to_string_lossy());
                cleaned = self.spellings.entry(key).or_insert(cleaned).clone();
            }
            clean = clean.join(&cleaned);
        }
        clean
    }

    pub(crate) fn push(&mut self, renamed: impl IntoIterator<Item = Renamed>) {
        self.list.extend(renamed);
    }

    pub(crate) fn into_list(self) -> Vec<Renamed> {
        self.list
    }
}

pub(crate) fn dedupe_names(files: &mut [PlannedFile], folders: &[VfsPath], rules: NameRules) -> Vec<Renamed> {
    let mut renamed = Vec::new();
    if rules == NameRules::Permissive {
        return renamed;
    }
    let mut taken: HashMap<VfsPath, HashSet<String>> = HashMap::new();
    for folder in folders {
        if let (Some(parent), Some(name)) = (folder.parent(), folder.file_name()) {
            taken.entry(parent).or_default().insert(rules.fold(&name.to_string_lossy()));
        }
    }
    for file in files.iter_mut() {
        let (Some(parent), Some(name)) = (file.destination.parent(), file.destination.file_name()) else {
            continue;
        };
        let name = name.to_string_lossy().into_owned();
        let folders_taken = taken.entry(parent.clone()).or_default();
        let name = if folders_taken.contains(&rules.fold(&name)) {
            let unique = unique_name(&name, folders_taken, rules);
            renamed.push(Renamed { from: name, to: unique.clone() });
            file.destination = parent.join(&unique);
            unique
        } else {
            name
        };
        folders_taken.insert(rules.fold(&name));
    }
    renamed
}

pub fn renamed_notice(renamed: &[Renamed]) -> String {
    let count = renamed.len();
    let plural = if count == 1 { "" } else { "s" };
    let example =
        renamed.first().map(|renamed| format!(", e.g. \"{}\" to \"{}\"", renamed.from, renamed.to)).unwrap_or_default();
    format!("Renamed {count} name{plural} the destination can't store{example}")
}

#[cfg(test)]
mod tests;

use porthmos_vfs::Entry;

use crate::i18n;

pub mod count;

pub const MAX_LISTED_NAMES: usize = 5;
pub fn this_computer() -> String {
    i18n::text("place.this_computer")
}

pub fn delete_summary(entries: &[Entry], place: &str, show: impl Fn(&str) -> String) -> Vec<String> {
    match entries {
        [] => Vec::new(),
        [only] if only.is_dir => {
            vec![i18n::text_with("delete.folder", &[("name", &show(&only.name)), ("place", place)])]
        }
        [only] => vec![i18n::text_with("delete.file", &[("name", &show(&only.name)), ("place", place)])],
        _ => {
            let count = entries.len().to_string();
            let mut lines = vec![i18n::text_with("delete.many", &[("count", &count), ("place", place)])];
            lines.extend(entries.iter().take(MAX_LISTED_NAMES).map(|entry| {
                let name = show(&entry.name);
                if entry.is_dir { format!("{name}/") } else { name }
            }));
            if entries.len() > MAX_LISTED_NAMES {
                let more = (entries.len() - MAX_LISTED_NAMES).to_string();
                lines.push(i18n::text_with("delete.more", &[("count", &more)]));
            }
            if entries.iter().any(|entry| entry.is_dir) {
                lines.push(i18n::text("delete.folder_note"));
            }
            lines
        }
    }
}

#[cfg(test)]
mod tests;

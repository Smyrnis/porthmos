use porthmos_vfs::Entry;

pub mod count;

pub const MAX_LISTED_NAMES: usize = 5;
pub const THIS_COMPUTER: &str = "this computer";

const FOLDER_NOTE: &str = "Folders are deleted with everything inside them.";

pub fn delete_summary(entries: &[Entry], place: &str, show: impl Fn(&str) -> String) -> Vec<String> {
    match entries {
        [] => Vec::new(),
        [only] if only.is_dir => {
            vec![format!("Delete the folder \"{}\" and everything inside it from {place}?", show(&only.name))]
        }
        [only] => vec![format!("Delete \"{}\" from {place}?", show(&only.name))],
        _ => {
            let mut lines = vec![format!("Delete {} items from {place}?", entries.len())];
            lines.extend(entries.iter().take(MAX_LISTED_NAMES).map(|entry| {
                let name = show(&entry.name);
                if entry.is_dir { format!("{name}/") } else { name }
            }));
            if entries.len() > MAX_LISTED_NAMES {
                lines.push(format!("and {} more", entries.len() - MAX_LISTED_NAMES));
            }
            if entries.iter().any(|entry| entry.is_dir) {
                lines.push(FOLDER_NOTE.to_string());
            }
            lines
        }
    }
}

#[cfg(test)]
mod tests;

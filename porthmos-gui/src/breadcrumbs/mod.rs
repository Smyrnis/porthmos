use porthmos_core::VfsPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub label: String,
    pub path: VfsPath,
}

const ROOT_LABEL: &str = "/";
const RELATIVE_LABEL: &str = ".";

fn is_drive(segment: &std::ffi::OsStr) -> bool {
    let text = segment.to_string_lossy();
    let mut characters = text.chars();
    matches!((characters.next(), characters.next(), characters.next()), (Some(letter), Some(':'), None) if letter.is_ascii_alphabetic())
}

pub fn crumbs(path: &VfsPath) -> Vec<Crumb> {
    if !path.is_absolute() {
        return vec![Crumb { label: RELATIVE_LABEL.to_string(), path: path.clone() }];
    }
    let starts_at_drive = path.segments().next().is_some_and(is_drive);
    let mut chain: Vec<VfsPath> = path.ancestors().collect();
    chain.reverse();
    chain
        .into_iter()
        .filter(|ancestor| !(starts_at_drive && ancestor.is_root()))
        .map(|ancestor| {
            let label = match ancestor.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => ROOT_LABEL.to_string(),
            };
            Crumb { label, path: ancestor }
        })
        .collect()
}

#[cfg(test)]
mod tests;

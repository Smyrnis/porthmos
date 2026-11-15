use crate::i18n;

const MAX_NAME_BYTES: usize = 255;
const RESERVED: [&str; 4] = ["con", "prn", "aux", "nul"];

fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_lowercase();
    let numbered =
        |prefix: &str| stem.strip_prefix(prefix).is_some_and(|rest| matches!(rest.as_bytes(), [b'1'..=b'9']));
    RESERVED.contains(&stem.as_str()) || numbered("com") || numbered("lpt")
}

pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        Some(i18n::text("names.empty"))
    } else if name == "." || name == ".." {
        Some(i18n::text("names.dots"))
    } else if name.contains('/') {
        Some(i18n::text("names.slash"))
    } else if name.chars().any(char::is_control) {
        Some(i18n::text("names.control"))
    } else if name.starts_with(' ') || name.ends_with(' ') {
        Some(i18n::text("names.space"))
    } else if name.len() > MAX_NAME_BYTES {
        Some(i18n::text_with("names.too_long", &[("max", &MAX_NAME_BYTES.to_string())]))
    } else if is_reserved(name) {
        Some(i18n::text("names.reserved"))
    } else {
        None
    }
}

pub fn entry_name_problem(name: &str, original: Option<&str>, exists: impl Fn(&str) -> bool) -> Option<String> {
    if original == Some(name) {
        return None;
    }
    if let Some(problem) = name_problem(name) {
        return Some(problem);
    }
    exists(name).then(|| i18n::text_with("names.exists", &[("name", name)]))
}

#[cfg(test)]
mod tests;

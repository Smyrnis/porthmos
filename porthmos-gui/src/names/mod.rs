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
        Some("A name can't be empty".to_string())
    } else if name == "." || name == ".." {
        Some("A name can't be \".\" or \"..\"".to_string())
    } else if name.contains('/') {
        Some("A name can't contain \"/\"".to_string())
    } else if name.chars().any(char::is_control) {
        Some("A name can't contain control characters".to_string())
    } else if name.starts_with(' ') || name.ends_with(' ') {
        Some("A name can't start or end with a space".to_string())
    } else if name.len() > MAX_NAME_BYTES {
        Some(format!("A name can't be longer than {MAX_NAME_BYTES} bytes"))
    } else if is_reserved(name) {
        Some("That name is reserved on Windows".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests;

use crate::profiles::labels::normalize_group;

pub fn printable(text: &str) -> String {
    text.chars().filter(|character| !character.is_control()).collect()
}

pub fn user_and_host(raw_host: &str, username: String) -> Result<(String, String), &'static str> {
    let trimmed = raw_host.trim();
    let (username, host) = match trimmed.rsplit_once('@') {
        Some((user, rest)) if username.is_empty() => {
            if user.contains(|character: char| character == ':' || character == '/' || character.is_whitespace()) {
                return Err("invalid host");
            }
            (printable(user).trim().to_string(), rest.trim().to_string())
        }
        _ => (username, trimmed.to_string()),
    };
    if host.is_empty() {
        return Err("no host");
    }
    if printable(&host) != host || host.contains(|character: char| character == '/' || character.is_whitespace()) {
        return Err("invalid host");
    }
    Ok((username, host))
}

pub fn name(text: &str) -> Option<String> {
    let cleaned = printable(text);
    let trimmed = cleaned.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn group_from_folders(folders: &[&str]) -> Option<String> {
    let cleaned: Vec<String> = folders.iter().filter_map(|folder| name(&folder.replace('/', "-"))).collect();
    normalize_group(&cleaned.join("/"))
}

#[cfg(test)]
mod tests;

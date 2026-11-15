use crate::profiles::labels::normalize_group;

const INVISIBLE: [char; 9] =
    ['\u{ad}', '\u{34f}', '\u{61c}', '\u{180e}', '\u{feff}', '\u{2028}', '\u{2029}', '\u{e0001}', '\u{206f}'];

fn hides_or_reorders_text(character: char) -> bool {
    matches!(character,
        '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}')
        || INVISIBLE.contains(&character)
}

pub fn printable(text: &str) -> String {
    text.chars().filter(|character| !character.is_control() && !hides_or_reorders_text(*character)).collect()
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

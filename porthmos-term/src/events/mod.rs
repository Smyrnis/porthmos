const MAX_TITLE_CHARACTERS: usize = 256;
const MAX_URI_PATH_BYTES: usize = 4096;
const FILE_PREFIX: &str = "file://";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingDirectory {
    pub host: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Bell,
    Title(String),
    ResetTitle,
    WorkingDirectory(WorkingDirectory),
    ClipboardStore(String),
    Exited(Option<u32>),
}

fn is_hidden_formatting(character: char) -> bool {
    matches!(
        character,
        '\u{ad}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
    )
}

pub fn sanitize_title(raw: &str) -> String {
    let visible: String = raw.chars().filter(|c| !c.is_control() && !is_hidden_formatting(*c)).collect();
    visible.trim().chars().take(MAX_TITLE_CHARACTERS).collect()
}

fn hex_value(digit: u8) -> Option<u8> {
    char::from(digit).to_digit(16).map(|value| value as u8)
}

fn percent_decode(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

pub fn parse_working_directory(uri: &str) -> Option<WorkingDirectory> {
    let rest = uri.strip_prefix(FILE_PREFIX)?;
    let slash = rest.find('/')?;
    let (host, encoded_path) = rest.split_at(slash);
    if encoded_path.len() > MAX_URI_PATH_BYTES
        || host.chars().any(|c| c.is_control() || c.is_whitespace() || is_hidden_formatting(c))
    {
        return None;
    }
    let path = percent_decode(encoded_path)?;
    if path.chars().any(|c| c.is_control() || is_hidden_formatting(c)) || path.split('/').any(|segment| segment == "..")
    {
        return None;
    }
    Some(WorkingDirectory { host: host.to_string(), path })
}

#[cfg(test)]
mod tests;

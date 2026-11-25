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

pub(crate) fn is_hidden_formatting(character: char) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryOwner {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryTracker {
    owner: DirectoryOwner,
    shell_host: Option<String>,
    directory: Option<String>,
}

impl DirectoryTracker {
    pub fn new(owner: DirectoryOwner) -> Self {
        Self { owner, shell_host: None, directory: None }
    }

    pub fn directory(&self) -> Option<&str> {
        self.directory.as_deref()
    }

    pub fn update(&mut self, report: &WorkingDirectory, local_names: &[String]) {
        let host = report.host.to_ascii_lowercase();
        let accepted = match self.owner {
            DirectoryOwner::Local => host.is_empty() || local_names.contains(&host),
            DirectoryOwner::Remote => &host == self.shell_host.get_or_insert_with(|| host.clone()),
        };
        self.directory = accepted.then(|| report.path.clone());
    }
}

#[cfg(unix)]
fn machine_host_name() -> Option<String> {
    let mut buffer = [0u8; 256];
    let status = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
    if status != 0 {
        return None;
    }
    let length = buffer.iter().position(|byte| *byte == 0).unwrap_or(buffer.len());
    String::from_utf8(buffer[..length].to_vec()).ok()
}

#[cfg(not(unix))]
fn machine_host_name() -> Option<String> {
    std::env::var("COMPUTERNAME").ok()
}

pub fn local_host_names() -> Vec<String> {
    let mut names = vec!["localhost".to_string()];
    if let Some(name) = machine_host_name().map(|name| name.to_ascii_lowercase()).filter(|name| !name.is_empty()) {
        names.push(name);
    }
    names
}

#[cfg(test)]
mod tests;

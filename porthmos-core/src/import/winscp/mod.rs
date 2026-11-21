use std::collections::BTreeMap;

use super::{
    Candidate, ImportError, ImportRow, Verdict, clean, escape,
    proxy::{ProxyFields, proxy_options},
};

const NOT_WINSCP: &str = "import.not_winscp";
const SESSIONS_PREFIX: &str = "Sessions\\";
const DEFAULT_SETTINGS: &str = "Default Settings";
const PPK_NOTE: &str = "key is in PuTTY format, convert it first";
const ALLOWED_KEYS: [&str; 12] = [
    "HostName",
    "PortNumber",
    "UserName",
    "FSProtocol",
    "Ftps",
    "RemoteDirectory",
    "PublicKeyFile",
    "IsWorkspace",
    "ProxyMethod",
    "ProxyHost",
    "ProxyPort",
    "ProxyUsername",
];
const PROXY_NOT_FOR_PROTOCOL: &str =
    "the proxy of this session is not imported: only SFTP and SCP connections can use one";

struct Section {
    raw_name: String,
    values: BTreeMap<&'static str, String>,
}

impl Section {
    fn value(&self, key: &str) -> Option<String> {
        self.values.get(key).map(|raw| escape::decode(raw))
    }
}

fn read_sections(text: &str) -> (bool, Vec<Section>) {
    let mut saw_sessions = false;
    let mut sections: Vec<Section> = Vec::new();
    let mut inside_session = false;
    for line in text.strip_prefix('\u{feff}').unwrap_or(text).lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            inside_session = false;
            if let Some(name) = header.strip_prefix(SESSIONS_PREFIX) {
                saw_sessions = true;
                if !name.is_empty() && !name.contains('\\') {
                    sections.push(Section { raw_name: name.to_string(), values: BTreeMap::new() });
                    inside_session = true;
                }
            }
            continue;
        }
        if !inside_session {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && let Some(allowed) = ALLOWED_KEYS.iter().find(|allowed| **allowed == key.trim())
            && let Some(section) = sections.last_mut()
        {
            section.values.insert(allowed, value.trim().to_string());
        }
    }
    (saw_sessions, sections)
}

fn unsupported(reason: &str) -> Verdict {
    Verdict::Unsupported(reason.to_string())
}

fn invalid(reason: &str) -> Verdict {
    Verdict::Invalid(reason.to_string())
}

fn protocol_of(section: &Section) -> Result<&'static str, Verdict> {
    let number = match section.value("FSProtocol") {
        None => 1,
        Some(text) => text.trim().parse::<i32>().map_err(|_| invalid("unknown protocol"))?,
    };
    match number {
        0 => Ok("scp"),
        1 | 2 => Ok("sftp"),
        5 => Ok("ftp"),
        6 => Ok("webdav"),
        7 => Ok("s3"),
        _ => Err(unsupported("this protocol is not supported")),
    }
}

fn security_of(section: &Section, protocol: &str) -> Result<Option<&'static str>, Verdict> {
    let ftps = match section.value("Ftps") {
        None => 0,
        Some(text) => text.trim().parse::<i32>().map_err(|_| invalid("unknown FTPS mode"))?,
    };
    match (protocol, ftps) {
        ("ftp", 0) => Ok(Some("plain")),
        ("ftp", 1) => Ok(Some("implicit")),
        ("ftp", 2 | 3) => Ok(Some("explicit")),
        ("ftp", _) => Err(invalid("unknown FTPS mode")),
        ("webdav", 0) => Ok(Some("http")),
        ("webdav", _) => Ok(Some("https")),
        _ => Ok(None),
    }
}

fn candidate(section: &Section, name: &str, folders: &[&str]) -> Result<Candidate, Verdict> {
    let protocol = protocol_of(section)?;
    let security = security_of(section, protocol)?;

    let raw_host = section.value("HostName").unwrap_or_default();
    let username = clean::printable(section.value("UserName").unwrap_or_default().trim()).trim().to_string();
    let (username, host) = clean::user_and_host(&raw_host, username).map_err(invalid)?;

    let port = match section.value("PortNumber").as_deref().map(str::trim) {
        None | Some("") | Some("0") => None,
        Some(text) => Some(text.parse::<u16>().map_err(|_| invalid("port"))?),
    };

    let mut options = BTreeMap::new();
    let mut notes = Vec::new();
    if let Some(security) = security {
        options.insert("security".to_string(), security.to_string());
    }
    if matches!(protocol, "sftp" | "scp")
        && let Some(key) =
            section.value("PublicKeyFile").map(|key| clean::printable(key.trim())).filter(|key| !key.is_empty())
    {
        if key.to_lowercase().ends_with(".ppk") {
            notes.push(PPK_NOTE.to_string());
        } else {
            options.insert("identity_file".to_string(), key);
        }
    }
    let proxy = proxy_options(&ProxyFields {
        method: section.value("ProxyMethod"),
        host: section.value("ProxyHost"),
        port: section.value("ProxyPort"),
        user: section.value("ProxyUsername"),
    });
    if matches!(protocol, "sftp" | "scp") {
        options.extend(proxy.options);
        notes.extend(proxy.notes);
    } else if !proxy.options.is_empty() {
        notes.push(PROXY_NOT_FOR_PROTOCOL.to_string());
    } else {
        notes.extend(proxy.notes);
    }
    if let Some(directory) =
        section.value("RemoteDirectory").map(|path| clean::printable(path.trim())).filter(|path| !path.is_empty())
    {
        options.insert("remote_path".to_string(), directory);
    }

    Ok(Candidate {
        name: name.to_string(),
        protocol: protocol.to_string(),
        host,
        port,
        username,
        group: clean::group_from_folders(folders),
        options,
        notes,
    })
}

fn row(section: &Section) -> ImportRow {
    let decoded = escape::decode(&section.raw_name);
    let (folder_path, last) = decoded.rsplit_once('/').unwrap_or(("", &decoded));
    let folders: Vec<&str> = if folder_path.is_empty() { Vec::new() } else { folder_path.split('/').collect() };
    let Some(name) = clean::name(last) else {
        return ImportRow { name: "(unnamed)".to_string(), candidate: None, verdict: invalid("no name") };
    };
    match candidate(section, &name, &folders) {
        Ok(found) => ImportRow { name, candidate: Some(found), verdict: Verdict::New },
        Err(verdict) => ImportRow { name, candidate: None, verdict },
    }
}

fn is_a_site(section: &Section) -> bool {
    section.values.contains_key("HostName")
        && section.value("IsWorkspace").as_deref() != Some("1")
        && escape::decode(&section.raw_name) != DEFAULT_SETTINGS
}

pub(crate) fn parse(text: &str) -> Result<Vec<ImportRow>, ImportError> {
    let (saw_sessions, sections) = read_sections(text);
    if !saw_sessions {
        return Err(ImportError::WrongFormat(NOT_WINSCP));
    }
    Ok(sections.iter().filter(|section| is_a_site(section)).map(row).collect())
}

#[cfg(test)]
mod tests;

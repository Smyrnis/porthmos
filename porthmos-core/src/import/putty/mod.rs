use crate::i18n;
use std::collections::BTreeMap;

use super::{Candidate, ImportError, ImportRow, Verdict, clean, escape};

const NOT_REG: &str = "import.not_reg";
const NOT_TEXT: &str = "import.not_text_reg";
const SESSIONS_KEY: &str = "hkey_current_user\\software\\simontatham\\putty\\sessions";
const DEFAULT_SETTINGS: &str = "Default Settings";
const PPK_NOTE: &str = "key is in PuTTY format, convert it first";
const ALLOWED_KEYS: [&str; 5] = ["HostName", "PortNumber", "UserName", "Protocol", "PublicKeyFile"];

#[derive(Default)]
struct Fields {
    host: Option<String>,
    port: Option<String>,
    username: Option<String>,
    protocol: Option<String>,
    key: Option<String>,
}

impl Fields {
    fn set(&mut self, key: &str, value: String) {
        match key {
            "HostName" => self.host = Some(value),
            "PortNumber" => self.port = Some(value),
            "UserName" => self.username = Some(value),
            "Protocol" => self.protocol = Some(value),
            "PublicKeyFile" => self.key = Some(value),
            _ => {}
        }
    }
}

fn unsupported(reason: &str) -> Verdict {
    Verdict::Unsupported(reason.to_string())
}

fn invalid(reason: &str) -> Verdict {
    Verdict::Invalid(reason.to_string())
}

fn unsupported_kind(protocol: &str) -> String {
    let kind = match protocol {
        "telnet" => "Telnet",
        "raw" => "Raw",
        "rlogin" => "Rlogin",
        "serial" => "Serial",
        _ => return i18n::text("import.other_kind_unsupported"),
    };
    i18n::text_with("import.kind_unsupported", &[("kind", kind)])
}

fn candidate(name: &str, fields: &Fields) -> Result<Candidate, Verdict> {
    let protocol = fields.protocol.as_deref().map(|text| text.trim().to_lowercase()).filter(|text| !text.is_empty());
    if let Some(protocol) = protocol.as_deref().filter(|protocol| *protocol != "ssh") {
        return Err(unsupported(&unsupported_kind(protocol)));
    }

    let raw_host = fields.host.as_deref().unwrap_or_default();
    let username = clean::printable(fields.username.as_deref().unwrap_or_default().trim()).trim().to_string();
    let (username, host) = clean::user_and_host(raw_host, username).map_err(invalid)?;

    let port = match fields.port.as_deref().map(str::trim) {
        None | Some("") | Some("0") => None,
        Some(text) => Some(text.parse::<u16>().map_err(|_| invalid("port"))?),
    };

    let mut options = BTreeMap::new();
    let mut notes = Vec::new();
    if let Some(key) = fields.key.as_deref().map(|key| clean::printable(key.trim())).filter(|key| !key.is_empty()) {
        if key.to_lowercase().ends_with(".ppk") {
            notes.push(PPK_NOTE.to_string());
        } else {
            options.insert("identity_file".to_string(), key);
        }
    }

    Ok(Candidate {
        name: name.to_string(),
        protocol: "sftp".to_string(),
        host,
        port,
        username,
        group: None,
        options,
        notes,
    })
}

fn row(decoded_name: &str, fields: &Fields) -> Option<ImportRow> {
    if decoded_name == DEFAULT_SETTINGS {
        return None;
    }
    let Some(name) = clean::name(decoded_name) else {
        return Some(ImportRow { name: "(unnamed)".to_string(), candidate: None, verdict: invalid("no name") });
    };
    Some(match candidate(&name, fields) {
        Ok(found) => ImportRow { name, candidate: Some(found), verdict: Verdict::New },
        Err(verdict) => ImportRow { name, candidate: None, verdict },
    })
}

pub(crate) fn parse_session(file_name: &str, text: &str) -> Option<ImportRow> {
    let mut fields = Fields::default();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=')
            && ALLOWED_KEYS.contains(&key)
        {
            fields.set(key, value.to_string());
        }
    }
    row(&escape::decode(file_name), &fields)
}

fn string_value(text: &str) -> Option<String> {
    let mut characters = text.strip_prefix('"')?.chars();
    let mut value = String::new();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Some(value),
            '\\' => match characters.next()? {
                escaped @ ('\\' | '"') => value.push(escaped),
                other => {
                    value.push('\\');
                    value.push(other);
                }
            },
            other => value.push(other),
        }
    }
    None
}

fn value_of(text: &str) -> Option<String> {
    if let Some(hex) = text.strip_prefix("dword:") {
        return u32::from_str_radix(hex.trim(), 16).ok().map(|number| number.to_string());
    }
    string_value(text)
}

pub(crate) fn parse_reg(text: &str) -> Result<Vec<ImportRow>, ImportError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let header = lines.next().unwrap_or("");
    if !header.eq_ignore_ascii_case("Windows Registry Editor Version 5.00") && !header.eq_ignore_ascii_case("REGEDIT4")
    {
        return Err(ImportError::WrongFormat(NOT_REG));
    }

    let prefix_with_separator = format!("{SESSIONS_KEY}\\");
    let mut saw_sessions = false;
    let mut sections: Vec<(String, Fields)> = Vec::new();
    let mut inside_session = false;
    for line in lines {
        if line.starts_with(';') {
            continue;
        }
        if let Some(key) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            inside_session = false;
            let lower = key.to_ascii_lowercase();
            if lower == SESSIONS_KEY {
                saw_sessions = true;
            } else if lower.starts_with(&prefix_with_separator) {
                saw_sessions = true;
                let name = &key[prefix_with_separator.len()..];
                if !name.is_empty() && !name.contains('\\') {
                    sections.push((name.to_string(), Fields::default()));
                    inside_session = true;
                }
            }
            continue;
        }
        if !inside_session {
            continue;
        }
        if let Some(rest) = line.strip_prefix('"')
            && let Some((key, value)) = rest.split_once("\"=")
            && ALLOWED_KEYS.contains(&key)
            && let Some(value) = value_of(value)
            && let Some((_, fields)) = sections.last_mut()
        {
            fields.set(key, value);
        }
    }
    if !saw_sessions {
        return Err(ImportError::WrongFormat(NOT_REG));
    }
    Ok(sections.iter().filter_map(|(name, fields)| row(&escape::decode(name), fields)).collect())
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> Result<String, ImportError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(ImportError::WrongFormat(NOT_TEXT));
    }
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| unit(*pair)).collect();
    String::from_utf16(&units).map_err(|_| ImportError::WrongFormat(NOT_TEXT))
}

fn utf8(bytes: &[u8]) -> Result<String, ImportError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| ImportError::WrongFormat(NOT_TEXT))
}

pub(crate) fn decode_reg_text(bytes: &[u8]) -> Result<String, ImportError> {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        [0xEF, 0xBB, 0xBF, rest @ ..] => utf8(rest),
        other => utf8(other),
    }
}

#[cfg(test)]
mod tests;

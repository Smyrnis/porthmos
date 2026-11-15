use std::{
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use roxmltree::{Document, Node, NodeId, ParsingOptions};

use super::{Candidate, ImportError, ImportRow, Verdict, clean};

const NOT_FILEZILLA: &str = "This is not a FileZilla site manager file";
const NODES_LIMIT: u32 = 2_000_000;
const MAX_FOLDER_DEPTH: usize = 32;
const LAST_PROTOCOL: i32 = 24;
const MAX_SEGMENT_LENGTH: usize = 32767;
const PPK_NOTE: &str = "key is in PuTTY format, convert it first";
const REMOTE_DIR_NOTE: &str = "remote folder not imported";
const ANONYMOUS_LOGON: i32 = 0;
const PROFILE_LOGON: i32 = 6;
const DEFAULT_CREDENTIALS_LOGON: i32 = 7;

fn child_text<'a>(node: Node<'a, '_>, tag: &str) -> Option<&'a str> {
    node.children().find(|child| child.has_tag_name(tag)).map(|child| child.text().unwrap_or("").trim())
}

fn own_text<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.children()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .map(str::trim)
        .find(|text| !text.is_empty())
}

struct FolderChains<'a> {
    known: HashMap<NodeId, Rc<Vec<&'a str>>>,
}

impl<'a> FolderChains<'a> {
    fn new() -> Self {
        Self { known: HashMap::new() }
    }

    fn above(&mut self, server: Node<'a, '_>) -> Rc<Vec<&'a str>> {
        let Some(start) = server.parent() else {
            return Rc::new(Vec::new());
        };
        let mut pending = Vec::new();
        let mut current = start;
        let mut chain = loop {
            if let Some(found) = self.known.get(&current.id()) {
                break found.clone();
            }
            if current.has_tag_name("Servers") {
                break Rc::new(Vec::new());
            }
            pending.push(current);
            match current.parent() {
                Some(parent) => current = parent,
                None => break Rc::new(Vec::new()),
            }
        };
        for node in pending.into_iter().rev() {
            if node.has_tag_name("Folder") {
                let mut names = chain.as_ref().clone();
                names.push(own_text(node).unwrap_or(""));
                if names.len() > MAX_FOLDER_DEPTH {
                    names.remove(0);
                }
                chain = Rc::new(names);
            }
            self.known.insert(node.id(), chain.clone());
        }
        chain
    }
}

fn unsupported(reason: &str) -> Verdict {
    Verdict::Unsupported(reason.to_string())
}

fn invalid(reason: &str) -> Verdict {
    Verdict::Invalid(reason.to_string())
}

fn map_protocol(number: i32) -> Result<(&'static str, Option<&'static str>), Verdict> {
    match number {
        1 => Ok(("sftp", None)),
        0 | 4 => Ok(("ftp", Some("explicit"))),
        3 => Ok(("ftp", Some("implicit"))),
        6 => Ok(("ftp", Some("plain"))),
        7 => Ok(("s3", None)),
        9 => Ok(("webdav", Some("https"))),
        19 => Ok(("webdav", Some("http"))),
        2 => Err(unsupported("HTTP sites are not supported")),
        5 => Err(unsupported("HTTPS sites are not supported")),
        8 => Err(unsupported("Storj is not supported")),
        _ => Err(unsupported("this storage service is not supported")),
    }
}

fn number(chars: &[char], at: &mut usize) -> Option<usize> {
    let start = *at;
    while chars.get(*at).is_some_and(char::is_ascii_digit) {
        *at += 1;
    }
    let digits: String = chars[start..*at].iter().collect();
    if digits.is_empty() || digits.len() > 5 { None } else { digits.parse().ok() }
}

fn space(chars: &[char], at: &mut usize) -> Option<()> {
    (chars.get(*at) == Some(&' ')).then(|| *at += 1)
}

fn unix_path_from_safe_path(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    if number(&chars, &mut at)? > 1 {
        return None;
    }
    space(&chars, &mut at)?;
    if number(&chars, &mut at)? != 0 {
        return None;
    }
    let mut segments = Vec::new();
    while at < chars.len() {
        space(&chars, &mut at)?;
        let length = number(&chars, &mut at)?;
        if length == 0 || length > MAX_SEGMENT_LENGTH {
            return None;
        }
        space(&chars, &mut at)?;
        let end = at.checked_add(length).filter(|end| *end <= chars.len())?;
        let segment: String = chars[at..end].iter().collect();
        if segment.contains('/') || clean::printable(&segment) != segment {
            return None;
        }
        segments.push(segment);
        at = end;
    }
    Some(format!("/{}", segments.join("/")))
}

fn candidate(server: Node<'_, '_>, display_name: &str, folders: &[&str]) -> Result<Candidate, Verdict> {
    let protocol_number = child_text(server, "Protocol")
        .and_then(|text| text.parse::<i32>().ok())
        .filter(|number| (0..=LAST_PROTOCOL).contains(number))
        .ok_or_else(|| invalid("unknown protocol"))?;
    let (protocol, security) = map_protocol(protocol_number)?;

    let host = child_text(server, "Host").unwrap_or("");
    if host.is_empty() {
        return Err(invalid("no host"));
    }
    if clean::printable(host) != host {
        return Err(invalid("invalid host"));
    }

    let logon = child_text(server, "Logontype").and_then(|text| text.parse::<i32>().ok()).unwrap_or(1);
    if logon == PROFILE_LOGON || logon == DEFAULT_CREDENTIALS_LOGON {
        return Err(unsupported("login with an AWS profile or default credentials is not supported"));
    }
    let username = if logon == ANONYMOUS_LOGON {
        if protocol != "ftp" {
            return Err(invalid("anonymous login"));
        }
        "anonymous".to_string()
    } else {
        clean::printable(child_text(server, "User").unwrap_or("")).trim().to_string()
    };

    let port = match child_text(server, "Port") {
        None | Some("") => None,
        Some(text) => match text.parse::<u16>() {
            Ok(port) if port != 0 => Some(port),
            _ => return Err(invalid("port")),
        },
    };

    let mut options = BTreeMap::new();
    let mut notes = Vec::new();
    if let Some(security) = security {
        options.insert("security".to_string(), security.to_string());
    }
    if protocol == "sftp"
        && let Some(key) = child_text(server, "Keyfile").map(clean::printable).filter(|key| !key.trim().is_empty())
    {
        if key.trim().to_lowercase().ends_with(".ppk") {
            notes.push(PPK_NOTE.to_string());
        } else {
            options.insert("identity_file".to_string(), key.trim().to_string());
        }
    }
    if let Some(safe_path) = child_text(server, "RemoteDir").filter(|text| !text.is_empty()) {
        match unix_path_from_safe_path(safe_path) {
            Some(path) => {
                options.insert("remote_path".to_string(), path);
            }
            None => notes.push(REMOTE_DIR_NOTE.to_string()),
        }
    }

    Ok(Candidate {
        name: display_name.to_string(),
        protocol: protocol.to_string(),
        host: host.to_string(),
        port,
        username,
        group: clean::group_from_folders(folders),
        options,
        notes,
    })
}

fn row<'a>(server: Node<'a, '_>, chains: &mut FolderChains<'a>) -> ImportRow {
    let name = child_text(server, "Name")
        .and_then(clean::name)
        .or_else(|| own_text(server).and_then(clean::name))
        .or_else(|| child_text(server, "Host").and_then(clean::name))
        .unwrap_or_else(|| "(unnamed)".to_string());
    let folders = chains.above(server);
    match candidate(server, &name, &folders) {
        Ok(found) => ImportRow { name, candidate: Some(found), verdict: Verdict::New },
        Err(verdict) => ImportRow { name, candidate: None, verdict },
    }
}

pub(crate) fn parse(text: &str) -> Result<Vec<ImportRow>, ImportError> {
    let options = ParsingOptions { allow_dtd: false, nodes_limit: NODES_LIMIT, ..ParsingOptions::default() };
    let document = Document::parse_with_options(text, options).map_err(|_| ImportError::WrongFormat(NOT_FILEZILLA))?;
    let root = document.root_element();
    if !root.has_tag_name("FileZilla3") {
        return Err(ImportError::WrongFormat(NOT_FILEZILLA));
    }
    let servers =
        root.children().find(|child| child.has_tag_name("Servers")).ok_or(ImportError::WrongFormat(NOT_FILEZILLA))?;
    let mut chains = FolderChains::new();
    Ok(servers
        .descendants()
        .filter(|node| node.has_tag_name("Server"))
        .map(|server| row(server, &mut chains))
        .collect())
}

#[cfg(test)]
mod tests;

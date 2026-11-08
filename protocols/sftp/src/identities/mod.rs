use std::collections::HashMap;

use russh_sftp::{client::SftpSession, protocol::FileAttributes};
use tokio::io::AsyncReadExt;

const PASSWD_PATH: &str = "/etc/passwd";
const GROUP_PATH: &str = "/etc/group";
const MAX_TABLE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Default)]
pub struct Identities {
    users: HashMap<u32, String>,
    groups: HashMap<u32, String>,
}

pub(crate) fn parse_names(table: &str) -> HashMap<u32, String> {
    let mut names = HashMap::new();
    for line in table.lines() {
        let mut fields = line.split(':');
        let (Some(name), Some(_), Some(id)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        let Ok(id) = id.parse::<u32>() else {
            continue;
        };
        if !name.is_empty() && !name.starts_with('#') {
            names.entry(id).or_insert_with(|| name.to_string());
        }
    }
    names
}

async fn read_table(sftp: &SftpSession, path: &str) -> String {
    let Ok(file) = sftp.open(path).await else {
        return String::new();
    };
    let mut bytes = Vec::new();
    if file.take(MAX_TABLE_BYTES).read_to_end(&mut bytes).await.is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn name_or_id(given: Option<&String>, known: &HashMap<u32, String>, id: Option<u32>) -> Option<String> {
    match given {
        Some(name) if !name.is_empty() => Some(name.clone()),
        _ => id.map(|id| known.get(&id).cloned().unwrap_or_else(|| id.to_string())),
    }
}

impl Identities {
    pub async fn load(sftp: &SftpSession) -> Self {
        let (passwd, group) = tokio::join!(read_table(sftp, PASSWD_PATH), read_table(sftp, GROUP_PATH));
        Self::from_tables(&passwd, &group)
    }

    pub fn from_tables(passwd: &str, group: &str) -> Self {
        Self { users: parse_names(passwd), groups: parse_names(group) }
    }

    pub fn ownership(&self, attributes: &FileAttributes) -> (Option<String>, Option<String>) {
        (
            name_or_id(attributes.user.as_ref(), &self.users, attributes.uid),
            name_or_id(attributes.group.as_ref(), &self.groups, attributes.gid),
        )
    }
}

#[cfg(test)]
mod tests;

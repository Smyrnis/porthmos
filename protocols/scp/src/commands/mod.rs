use anyhow::anyhow;
use porthmos_ssh::shell_quote;
use porthmos_vfs::{ErrorKind, ProtocolError, VfsPath};

pub(crate) const PROBE: &str =
    "printf 'porthmos\\n%s\\n' \"$HOME\"; if command -v scp >/dev/null 2>&1; then echo scp; fi";
pub(crate) const PROBE_MARKER: &str = "porthmos";
const LISTING_ENVIRONMENT: &str = "TZ=UTC0 LC_ALL=C QUOTING_STYLE=literal TIME_STYLE=+%s";

fn quoted(path: &VfsPath) -> String {
    shell_quote(&path.to_string_lossy())
}

pub(crate) fn list(dir: &VfsPath) -> String {
    let mut text = dir.to_string_lossy().into_owned();
    if !text.ends_with('/') {
        text.push('/');
    }
    format!("{LISTING_ENVIRONMENT} ls -lanL -- {}", shell_quote(&text))
}

pub(crate) fn stat(path: &VfsPath) -> String {
    format!("{LISTING_ENVIRONMENT} ls -ldnL -- {}", quoted(path))
}

pub(crate) fn free_space(path: &VfsPath) -> String {
    format!("LC_ALL=C df -Pk -- {}", quoted(path))
}

pub(crate) fn mkdir(path: &VfsPath) -> String {
    format!("mkdir -- {}", quoted(path))
}

pub(crate) fn touch(path: &VfsPath, seconds: u64) -> String {
    format!("test -e {path} && touch -c -d @{seconds} -- {path}", path = quoted(path))
}

pub(crate) fn remove(path: &VfsPath) -> String {
    format!("rm -- {}", quoted(path))
}

pub(crate) fn remove_dir(path: &VfsPath) -> String {
    format!("rmdir -- {}", quoted(path))
}

pub(crate) fn remove_tree(path: &VfsPath) -> String {
    format!("rm -rf -- {}", quoted(path))
}

pub(crate) fn rename(from: &VfsPath, to: &VfsPath) -> String {
    format!("mv -f -- {} {}", quoted(from), quoted(to))
}

pub(crate) fn read_from(path: &VfsPath, offset: u64) -> String {
    if offset == 0 {
        format!("cat -- {}", quoted(path))
    } else {
        format!("tail -c +{} -- {}", offset + 1, quoted(path))
    }
}

pub(crate) fn scp_source(path: &VfsPath) -> String {
    format!("scp -f -- {}", quoted(path))
}

pub(crate) fn scp_sink(path: &VfsPath) -> String {
    format!(": > {} && scp -t -- {}", quoted(path), quoted(path))
}

pub(crate) fn append(path: &VfsPath) -> String {
    format!("cat >> {}", quoted(path))
}

pub(crate) fn create(path: &VfsPath) -> String {
    format!("cat > {}", quoted(path))
}

pub(crate) fn failure(stderr: &[u8], status: Option<u32>, path: &VfsPath, command: &str) -> ProtocolError {
    let text = String::from_utf8_lossy(stderr);
    if ["No such file or directory", "Directory nonexistent", "nonexistent directory"]
        .iter()
        .any(|hint| text.contains(hint))
    {
        return ProtocolError::new(ErrorKind::NotFound, anyhow!("{path} not found"));
    }
    if ["Permission denied", "Operation not permitted", "Read-only file system"].iter().any(|hint| text.contains(hint))
    {
        return ProtocolError::new(ErrorKind::PermissionDenied, anyhow!("{path}: permission denied"));
    }
    if text.contains("File exists") {
        return ProtocolError::new(ErrorKind::Other, anyhow!("{path} already exists"));
    }
    match text.lines().map(str::trim).find(|line| !line.is_empty()) {
        Some(first) => ProtocolError::new(ErrorKind::Other, anyhow!(first.to_string())),
        None => {
            let exit = status.map_or_else(|| "no status".to_string(), |code| code.to_string());
            ProtocolError::new(ErrorKind::Other, anyhow!("{command} failed (exit {exit})"))
        }
    }
}

#[cfg(test)]
mod tests;

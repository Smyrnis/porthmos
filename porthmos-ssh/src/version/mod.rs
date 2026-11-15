use std::{
    process::{Command, Stdio},
    sync::OnceLock,
};

const MARKER: &str = "OpenSSH_";
const WINDOWS_MARKER: &str = "for_Windows_";
const FIRST_ASKPASS_VERSION: SshVersion = SshVersion { major: 8, minor: 4 };

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SshVersion {
    pub major: u32,
    pub minor: u32,
}

impl SshVersion {
    pub fn supports_askpass_password(self) -> bool {
        self >= FIRST_ASKPASS_VERSION
    }
}

fn leading_number(text: &str) -> Option<(u32, &str)> {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    let number = text[..digits].parse().ok()?;
    Some((number, &text[digits..]))
}

pub fn parse_ssh_version(output: &str) -> Option<SshVersion> {
    let after_marker = &output[output.find(MARKER)? + MARKER.len()..];
    let numbers = after_marker.strip_prefix(WINDOWS_MARKER).unwrap_or(after_marker);
    let (major, rest) = leading_number(numbers)?;
    let minor = rest.strip_prefix('.').and_then(leading_number).map_or(0, |(minor, _)| minor);
    Some(SshVersion { major, minor })
}

fn ask_installed_ssh() -> Option<SshVersion> {
    let output = Command::new("ssh").arg("-V").stdin(Stdio::null()).output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ssh_version(&stderr).or_else(|| parse_ssh_version(&stdout))
}

pub fn installed_ssh_version() -> Option<SshVersion> {
    static CACHE: OnceLock<Option<SshVersion>> = OnceLock::new();
    *CACHE.get_or_init(ask_installed_ssh)
}

#[cfg(test)]
mod tests;

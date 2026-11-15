use std::path::{Path, PathBuf};

fn encode(path: &Path) -> String {
    let mut out = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => out.push(byte as char),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

pub fn list(paths: &[PathBuf]) -> String {
    paths.iter().map(|path| format!("{}\r\n", encode(path))).collect()
}

fn decode(text: &str) -> Option<PathBuf> {
    let rest = text.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 3 <= bytes.len() {
            let value = u8::from_str_radix(std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?, 16).ok()?;
            out.push(value);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

pub fn parse(text: &str) -> Vec<PathBuf> {
    text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).filter_map(decode).collect()
}

#[cfg(test)]
mod tests;

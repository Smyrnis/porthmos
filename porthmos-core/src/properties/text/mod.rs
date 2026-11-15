use super::Measured;
use crate::{
    delete_prompt::count::{counted, grouped},
    size_text::size_text,
};

const PLAIN_BYTES_LIMIT: u64 = 1024;

pub fn size_line(bytes: u64) -> String {
    if bytes < PLAIN_BYTES_LIMIT {
        return format!("{bytes} B");
    }
    format!("{} ({} bytes)", size_text(bytes), grouped(bytes))
}

pub fn contents_line(measured: &Measured) -> String {
    let parts: Vec<String> = [counted(measured.files, "file", "files"), counted(measured.folders, "folder", "folders")]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() { "nothing inside".to_string() } else { parts.join(", ") }
}

pub fn skipped_line(measured: &Measured) -> Option<String> {
    let parts: Vec<String> = [
        counted(measured.symlinks, "link not followed", "links not followed"),
        counted(measured.unreadable, "folder not readable", "folders not readable"),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() { None } else { Some(parts.join(", ")) }
}

pub fn progress_line(measured: &Measured) -> String {
    let parts: Vec<String> = [counted(measured.files, "file", "files"), counted(measured.folders, "folder", "folders")]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() {
        "measuring\u{2026}".to_string()
    } else {
        format!("measuring\u{2026} {} so far", parts.join(" and "))
    }
}

#[cfg(test)]
mod tests;

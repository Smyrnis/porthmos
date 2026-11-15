use super::Measured;
use crate::i18n;
use crate::{
    delete_prompt::count::{counted, grouped, joined_and},
    size_text::size_text,
};

const PLAIN_BYTES_LIMIT: u64 = 1024;

pub fn size_line(bytes: u64) -> String {
    if bytes < PLAIN_BYTES_LIMIT {
        return format!("{bytes} B");
    }
    i18n::text_with("properties.size_detail", &[("size", &size_text(bytes)), ("bytes", &grouped(bytes))])
}

pub fn contents_line(measured: &Measured) -> String {
    let parts: Vec<String> = [counted(measured.files, "count.files"), counted(measured.folders, "count.folders")]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() { i18n::text("properties.nothing_inside") } else { parts.join(", ") }
}

pub fn skipped_line(measured: &Measured) -> Option<String> {
    let parts: Vec<String> =
        [counted(measured.symlinks, "count.links_skipped"), counted(measured.unreadable, "count.folders_unreadable")]
            .into_iter()
            .flatten()
            .collect();
    if parts.is_empty() { None } else { Some(parts.join(", ")) }
}

pub fn progress_line(measured: &Measured) -> String {
    let parts: Vec<String> = [counted(measured.files, "count.files"), counted(measured.folders, "count.folders")]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() {
        i18n::text("properties.measuring")
    } else {
        i18n::text_with("properties.measuring_so_far", &[("what", &joined_and(parts))])
    }
}

#[cfg(test)]
mod tests;

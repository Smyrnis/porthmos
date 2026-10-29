use std::sync::Arc;

use porthmos_core::edit::printable;
use porthmos_core::preview::{Preview, PreviewContent};

use crate::view::format::size;

const HEX_BYTES: usize = 16;
const TAB: usize = 4;
const OVERSCAN: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewStatus {
    Loading,
    Ready(Arc<Preview>),
    Failed(String),
}

#[derive(Debug)]
pub struct PreviewState {
    request_id: u64,
    name: String,
    status: PreviewStatus,
    lines: Vec<String>,
}

fn expand_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut column = 0;
    for character in line.chars() {
        match character {
            '\t' => {
                let stop = TAB - column % TAB;
                out.extend(std::iter::repeat_n(' ', stop));
                column += stop;
            }
            character if character.is_control() => {
                out.push('?');
                column += 1;
            }
            character => {
                out.push(character);
                column += 1;
            }
        }
    }
    out
}

pub fn text_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines.into_iter().map(|line| expand_line(line.strip_suffix('\r').unwrap_or(line))).collect()
}

pub fn hex_row(offset: usize, bytes: &[u8]) -> String {
    let mut row = format!("{offset:08x}  ");
    for index in 0..HEX_BYTES {
        if index == HEX_BYTES / 2 {
            row.push(' ');
        }
        match bytes.get(index) {
            Some(byte) => row.push_str(&format!("{byte:02x} ")),
            None => row.push_str("   "),
        }
    }
    row.push_str(" |");
    row.extend(
        bytes.iter().take(HEX_BYTES).map(|byte| if (0x20..0x7f).contains(byte) { char::from(*byte) } else { '.' }),
    );
    row.push('|');
    row
}

pub fn size_text(bytes: u64) -> String {
    if bytes < 1024 { format!("{bytes} B") } else { size(bytes, false) }
}

pub fn visible_window(top: f32, viewport: f32, rows: usize, row_height: f32) -> (usize, usize) {
    if rows == 0 || row_height <= 0.0 {
        return (0, 0);
    }
    let first = ((top.max(0.0) / row_height) as usize).saturating_sub(OVERSCAN).min(rows);
    let last = (((top.max(0.0) + viewport.max(0.0)) / row_height).ceil() as usize + OVERSCAN).min(rows);
    (first, last.saturating_sub(first))
}

impl PreviewState {
    pub fn loading(request_id: u64, name: String) -> Self {
        Self { request_id, name, status: PreviewStatus::Loading, lines: Vec::new() }
    }

    #[cfg(test)]
    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    pub fn status(&self) -> &PreviewStatus {
        &self.status
    }

    pub fn is_loading(&self) -> bool {
        self.status == PreviewStatus::Loading
    }

    pub fn finish(&mut self, request_id: u64, result: Result<Arc<Preview>, String>) -> bool {
        if request_id != self.request_id || !self.is_loading() {
            return false;
        }
        self.status = match result {
            Ok(preview) => {
                if let PreviewContent::Text(text) = &preview.content {
                    self.lines = text_lines(text);
                }
                PreviewStatus::Ready(preview)
            }
            Err(message) => PreviewStatus::Failed(message),
        };
        true
    }

    pub fn title(&self) -> String {
        printable(&self.name)
    }

    fn ready(&self) -> Option<&Preview> {
        match &self.status {
            PreviewStatus::Ready(preview) => Some(preview),
            _ => None,
        }
    }

    pub fn is_binary(&self) -> bool {
        self.ready().is_some_and(|preview| matches!(preview.content, PreviewContent::Binary(_)))
    }

    pub fn info_text(&self) -> String {
        let Some(preview) = self.ready() else {
            return String::new();
        };
        if preview.truncated {
            return match preview.size {
                Some(total) => format!("first {} of {}", size_text(preview.shown as u64), size_text(total)),
                None => format!("first {}", size_text(preview.shown as u64)),
            };
        }
        let kind = if self.is_binary() { "binary" } else { "text" };
        format!("{kind} · {}", size_text(preview.size.unwrap_or(preview.shown as u64)))
    }

    pub fn row_count(&self) -> usize {
        match self.ready().map(|preview| &preview.content) {
            Some(PreviewContent::Text(_)) => self.lines.len(),
            Some(PreviewContent::Binary(bytes)) => bytes.len().div_ceil(HEX_BYTES),
            None => 0,
        }
    }

    pub fn columns(&self) -> usize {
        match self.ready().map(|preview| &preview.content) {
            Some(PreviewContent::Text(_)) => self.lines.iter().map(|line| line.chars().count()).max().unwrap_or(0),
            Some(PreviewContent::Binary(_)) => hex_row(0, &[0; HEX_BYTES]).chars().count(),
            None => 0,
        }
    }

    pub fn rows(&self, first: usize, count: usize) -> Vec<String> {
        let end = first.saturating_add(count).min(self.row_count());
        if first >= end {
            return Vec::new();
        }
        match self.ready().map(|preview| &preview.content) {
            Some(PreviewContent::Text(_)) => self.lines[first..end].to_vec(),
            Some(PreviewContent::Binary(bytes)) => (first..end)
                .map(|row| {
                    let from = row * HEX_BYTES;
                    hex_row(from, &bytes[from..(from + HEX_BYTES).min(bytes.len())])
                })
                .collect(),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;

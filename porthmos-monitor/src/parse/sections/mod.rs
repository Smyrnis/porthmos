mod cpu;
mod disks;
mod host;
mod memory;
mod network;
mod processes;

pub(crate) use cpu::{CpuTicks, cpu, percent};
pub(crate) use disks::disks;
pub(crate) use host::{host, load};
pub(crate) use memory::memory;
pub(crate) use network::{Counters, counters, interfaces};
pub(crate) use processes::{History, Inputs, processes};

pub fn hides_or_reorders_text(character: char) -> bool {
    matches!(
        character,
        '\u{ad}'
            | '\u{180e}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{e0001}'
            | '\u{34f}'
            | '\u{61c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
    )
}

pub(crate) fn text(bytes: &[u8], limit: usize) -> Option<String> {
    let cleaned: String = String::from_utf8_lossy(bytes)
        .chars()
        .filter(|character| !character.is_control() && !hides_or_reorders_text(*character))
        .collect();
    let trimmed = cleaned.trim();
    (!trimmed.is_empty()).then(|| trimmed.chars().take(limit).collect())
}

pub(crate) fn finite(text: &str) -> Option<f32> {
    text.parse::<f32>().ok().filter(|value| value.is_finite() && *value >= 0.0)
}

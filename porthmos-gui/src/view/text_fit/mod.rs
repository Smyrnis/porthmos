use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const REPLACEMENT: char = '·';
const ELLIPSIS: char = '…';

fn is_invisible_format(character: char) -> bool {
    matches!(character, '\u{61c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn visible(character: char) -> char {
    if character.is_control() || is_invisible_format(character) { REPLACEMENT } else { character }
}

pub fn fit(name: &str, max_cells: usize) -> String {
    if max_cells == 0 {
        return String::new();
    }
    let cleaned: String = name.chars().map(visible).collect();
    if cleaned.width() <= max_cells {
        return cleaned;
    }
    let ends: Vec<usize> = std::iter::once(0)
        .chain(cleaned.char_indices().map(|(index, character)| index + character.len_utf8()))
        .take(max_cells.saturating_mul(2).saturating_add(3))
        .collect();
    let fits = |kept: usize| cleaned[..ends[kept]].width() < max_cells;
    let mut estimate = 0;
    let mut kept = cleaned
        .chars()
        .take(ends.len() - 1)
        .position(|character| {
            estimate += character.width().unwrap_or(0);
            estimate >= max_cells
        })
        .unwrap_or(ends.len() - 1);
    while kept > 0 && !fits(kept) {
        kept -= 1;
    }
    while kept + 1 < ends.len() && fits(kept + 1) {
        kept += 1;
    }
    let mut shown = cleaned[..ends[kept]].to_string();
    shown.push(ELLIPSIS);
    shown
}

pub fn full_text_hint(text: &str, max_cells: usize) -> Option<String> {
    (fit(text, max_cells) != text).then(|| text.chars().map(visible).collect())
}

#[cfg(test)]
mod tests;

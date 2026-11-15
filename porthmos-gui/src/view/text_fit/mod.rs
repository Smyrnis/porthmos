use unicode_width::UnicodeWidthStr;

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
    let mut shown = String::new();
    for character in cleaned.chars().take(max_cells.saturating_mul(2).saturating_add(2)) {
        shown.push(character);
        if shown.width() + 1 > max_cells {
            shown.pop();
            break;
        }
    }
    shown.push(ELLIPSIS);
    shown
}

#[cfg(test)]
mod tests;

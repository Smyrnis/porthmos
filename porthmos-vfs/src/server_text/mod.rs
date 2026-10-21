use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const SERVER_INFO_CELLS: usize = 40;
const ELLIPSIS: char = '\u{2026}';

fn is_hidden(character: char) -> bool {
    character.is_control()
        || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{61c}')
}

pub fn clean_server_text(text: &str, max_cells: usize) -> Option<String> {
    if max_cells == 0 {
        return None;
    }
    let spaced: String = text.chars().map(|character| if is_hidden(character) { ' ' } else { character }).collect();
    let collapsed = spaced.split_whitespace().collect::<Vec<&str>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if collapsed.width() <= max_cells {
        return Some(collapsed);
    }
    let mut cut = String::new();
    let mut used = 0;
    for character in collapsed.chars() {
        let width = character.width().unwrap_or(0);
        if used + width > max_cells - 1 {
            break;
        }
        used += width;
        cut.push(character);
    }
    cut.push(ELLIPSIS);
    Some(cut)
}

#[cfg(test)]
mod tests;

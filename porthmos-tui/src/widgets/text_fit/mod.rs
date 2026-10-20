use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const ELLIPSIS: char = '\u{2026}';

pub fn take_width(text: &str, width: usize) -> String {
    let mut used = 0;
    let mut taken = String::new();
    for character in text.chars() {
        let character_width = character.width().unwrap_or(0);
        if used + character_width > width {
            break;
        }
        used += character_width;
        taken.push(character);
    }
    taken
}

pub fn truncate_with_ellipsis(text: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    if text.width() <= budget {
        return text.to_string();
    }
    let mut kept = take_width(text, budget - 1);
    kept.push(ELLIPSIS);
    kept
}

pub fn truncate_start_with_ellipsis(text: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    if text.width() <= budget {
        return text.to_string();
    }
    let mut used = 0;
    let mut tail: Vec<char> = Vec::new();
    for character in text.chars().rev() {
        let character_width = character.width().unwrap_or(0);
        if used + character_width > budget - 1 {
            break;
        }
        used += character_width;
        tail.push(character);
    }
    tail.push(ELLIPSIS);
    tail.reverse();
    tail.into_iter().collect()
}

#[cfg(test)]
mod tests;

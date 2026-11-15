const PERMISSION_BITS: u32 = 0o7777;
const TRIPLETS: [(u32, char); 9] = [
    (0o400, 'r'),
    (0o200, 'w'),
    (0o100, 'x'),
    (0o040, 'r'),
    (0o020, 'w'),
    (0o010, 'x'),
    (0o004, 'r'),
    (0o002, 'w'),
    (0o001, 'x'),
];
const SPECIAL_BITS: [(u32, usize, char); 3] = [(0o4000, 2, 's'), (0o2000, 5, 's'), (0o1000, 8, 't')];

pub fn parse_mode(text: &str) -> Result<u32, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(crate::i18n::text("permissions.mode.empty"));
    }
    if !text.chars().all(|digit| ('0'..='7').contains(&digit)) {
        return Err(crate::i18n::text("permissions.mode.not_octal"));
    }
    if !(3..=4).contains(&text.len()) {
        return Err(crate::i18n::text("permissions.mode.wrong_length"));
    }
    u32::from_str_radix(text, 8).map_err(|_| crate::i18n::text("permissions.mode.not_octal"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeRequest {
    pub mode: u32,
    pub recursive: bool,
}

pub fn parse_mode_request(text: &str) -> Result<ModeRequest, String> {
    let mut recursive = false;
    let mut mode_words: Vec<&str> = Vec::new();
    for word in text.split_whitespace() {
        if word.eq_ignore_ascii_case("-r") {
            if recursive {
                return Err(crate::i18n::text("permissions.request.flag_twice"));
            }
            recursive = true;
        } else if word.len() > 1 && word.starts_with('-') && !word[1..].chars().all(|digit| digit.is_ascii_digit()) {
            return Err(crate::i18n::text("permissions.request.unknown_option"));
        } else {
            mode_words.push(word);
        }
    }
    if mode_words.len() > 1 {
        return Err(crate::i18n::text("permissions.request.two_modes"));
    }
    let mode = parse_mode(mode_words.first().copied().unwrap_or(""))?;
    Ok(ModeRequest { mode, recursive })
}

pub fn mode_text(mode: u32) -> String {
    format!("{:03o}", mode & PERMISSION_BITS)
}

pub fn symbolic(mode: u32) -> String {
    let mut letters: Vec<char> =
        TRIPLETS.iter().map(|(bit, letter)| if mode & bit != 0 { *letter } else { '-' }).collect();
    for (bit, index, set) in SPECIAL_BITS {
        if mode & bit != 0 {
            letters[index] = if letters[index] == 'x' { set } else { set.to_ascii_uppercase() };
        }
    }
    letters.into_iter().collect()
}

#[cfg(test)]
mod tests;

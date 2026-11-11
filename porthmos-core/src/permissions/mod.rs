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
        return Err("Type a mode such as 644 or 755".to_string());
    }
    if !text.chars().all(|digit| ('0'..='7').contains(&digit)) {
        return Err("A mode is made of the digits 0 to 7".to_string());
    }
    if !(3..=4).contains(&text.len()) {
        return Err("A mode has three or four digits".to_string());
    }
    u32::from_str_radix(text, 8).map_err(|_| "A mode is made of the digits 0 to 7".to_string())
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

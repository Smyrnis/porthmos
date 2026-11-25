pub const PREVIEW_LINES: usize = 5;
pub const PREVIEW_LINE_CHARACTERS: usize = 80;

fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn needs_paste_confirmation(text: &str, bracketed: bool) -> bool {
    !bracketed && text.chars().any(|character| character.is_control() && character != '\t')
}

pub fn paste_line_count(text: &str) -> usize {
    normalized(text).lines().count()
}

fn escaped(character: char) -> String {
    match character {
        '\t' => "\t".to_string(),
        '\u{7f}' => "^?".to_string(),
        character if (character as u32) < 0x20 => format!("^{}", char::from(character as u8 + 0x40)),
        character if ('\u{80}'..='\u{9f}').contains(&character) => format!("\\u{{{:x}}}", character as u32),
        character => character.to_string(),
    }
}

fn clipped_line(line: &str) -> String {
    let shown: String = line.chars().take(PREVIEW_LINE_CHARACTERS).collect();
    let clipped = line.chars().nth(PREVIEW_LINE_CHARACTERS).is_some();
    let mut text: String = shown.chars().map(escaped).collect();
    if clipped {
        text.push('…');
    }
    text
}

pub fn paste_preview(text: &str) -> String {
    let normal = normalized(text);
    let mut lines: Vec<String> = normal.lines().take(PREVIEW_LINES).map(clipped_line).collect();
    if normal.lines().nth(PREVIEW_LINES).is_some() {
        lines.push("…".to_string());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;

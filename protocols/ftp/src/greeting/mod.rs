use porthmos_vfs::{SERVER_INFO_CELLS, clean_server_text};

fn without_status_code(line: &str) -> &str {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 3 && line[digits..].starts_with([' ', '-']) { &line[digits + 1..] } else { line }
}

pub(crate) fn greeting_info(welcome: Option<&str>) -> Option<String> {
    let first = welcome?.lines().map(str::trim).find(|line| !line.is_empty())?;
    let text = if first.len() == 3 && first.chars().all(|character| character.is_ascii_digit()) {
        ""
    } else {
        without_status_code(first)
    };
    clean_server_text(text, SERVER_INFO_CELLS)
}

#[cfg(test)]
mod tests;

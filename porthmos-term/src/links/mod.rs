use std::ops::Range;

const SCHEMES: [&str; 3] = ["https://", "http://", "mailto:"];
const BOUNDARIES: &str = "<>\"'`";
const TRAILING: &str = ".,;:!?'\"]}";

pub fn is_openable(uri: &str) -> bool {
    if uri.chars().any(|character| {
        character.is_control() || character.is_whitespace() || crate::events::is_hidden_formatting(character)
    }) {
        return false;
    }
    let Some((scheme, rest)) = uri.split_once(':') else {
        return false;
    };
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" => {
            rest.strip_prefix("//").is_some_and(|authority| !authority.is_empty() && !authority.starts_with('/'))
        }
        "mailto" => !rest.is_empty(),
        _ => false,
    }
}

fn is_boundary(character: char) -> bool {
    character.is_whitespace() || BOUNDARIES.contains(character)
}

fn scheme_offset(token: &[char]) -> Option<usize> {
    let lowered: String = token.iter().map(char::to_ascii_lowercase).collect();
    SCHEMES.iter().filter_map(|scheme| lowered.find(scheme)).min().map(|byte| lowered[..byte].chars().count())
}

fn trim_end(characters: &[char], start: usize, mut end: usize) -> usize {
    while end > 0 {
        let last = characters[end - 1];
        let unbalanced_parenthesis = last == ')'
            && characters[start..end].iter().filter(|c| **c == ')').count()
                > characters[start..end].iter().filter(|c| **c == '(').count();
        if TRAILING.contains(last) || unbalanced_parenthesis {
            end -= 1;
        } else {
            break;
        }
    }
    end
}

pub fn url_at(text: &str, column: usize) -> Option<Range<usize>> {
    let characters: Vec<char> = text.chars().collect();
    if column >= characters.len() || is_boundary(characters[column]) {
        return None;
    }
    let mut start = column;
    while start > 0 && !is_boundary(characters[start - 1]) {
        start -= 1;
    }
    let mut end = column + 1;
    while end < characters.len() && !is_boundary(characters[end]) {
        end += 1;
    }
    let start = start + scheme_offset(&characters[start..end])?;
    let end = trim_end(&characters, start, end);
    let url: String = characters.get(start..end)?.iter().collect();
    (start <= column && column < end && is_openable(&url)).then_some(start..end)
}

#[cfg(test)]
mod tests;

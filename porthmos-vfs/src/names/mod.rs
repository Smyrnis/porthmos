use std::fmt;

const RESERVED_STEMS: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const REPLACEMENT: char = '_';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NameRules {
    #[default]
    Permissive,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameProblem {
    Empty,
    ForbiddenCharacter(char),
    ReservedDeviceName(String),
    TrailingDotOrSpace,
}

impl fmt::Display for NameProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameProblem::Empty => write!(f, "is empty"),
            NameProblem::ForbiddenCharacter(character) => write!(f, "contains '{}'", character.escape_debug()),
            NameProblem::ReservedDeviceName(_) => write!(f, "is a reserved device name"),
            NameProblem::TrailingDotOrSpace => write!(f, "ends with a dot or a space"),
        }
    }
}

fn is_forbidden(character: char) -> bool {
    FORBIDDEN.contains(&character) || character.is_control() && (character as u32) < 0x20
}

fn is_reserved_device(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    RESERVED_STEMS.contains(&upper.as_str())
        || ["COM", "LPT"]
            .iter()
            .any(|prefix| upper.strip_prefix(prefix).is_some_and(|digit| matches!(digit.as_bytes(), [b'1'..=b'9'])))
}

impl NameRules {
    pub fn problem(self, name: &str) -> Option<NameProblem> {
        match self {
            NameRules::Permissive => None,
            NameRules::Windows => {
                if name.is_empty() {
                    return Some(NameProblem::Empty);
                }
                if let Some(character) = name.chars().find(|character| is_forbidden(*character)) {
                    return Some(NameProblem::ForbiddenCharacter(character));
                }
                if is_reserved_device(name) {
                    return Some(NameProblem::ReservedDeviceName(name.to_string()));
                }
                name.ends_with(['.', ' ']).then_some(NameProblem::TrailingDotOrSpace)
            }
        }
    }

    pub fn sanitize(self, name: &str) -> String {
        match self {
            NameRules::Permissive => name.to_string(),
            NameRules::Windows => {
                let replaced: String = name
                    .chars()
                    .map(|character| if is_forbidden(character) { REPLACEMENT } else { character })
                    .collect();
                let trimmed = replaced.trim_end_matches(['.', ' ']);
                let named = if trimmed.is_empty() { REPLACEMENT.to_string() } else { trimmed.to_string() };
                if is_reserved_device(&named) { format!("{REPLACEMENT}{named}") } else { named }
            }
        }
    }

    pub fn fold(self, name: &str) -> String {
        match self {
            NameRules::Permissive => name.to_string(),
            NameRules::Windows => name.to_lowercase(),
        }
    }

    pub fn is_case_sensitive(self) -> bool {
        self == NameRules::Permissive
    }
}

#[cfg(test)]
mod tests;

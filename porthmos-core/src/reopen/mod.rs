use crate::i18n;

const LISTED: usize = 3;

pub fn offer_summary(names: &[String]) -> String {
    let listed = names.iter().take(LISTED).map(String::as_str).collect::<Vec<_>>().join(", ");
    match names.len().saturating_sub(LISTED) {
        0 => listed,
        more => format!("{listed} {}", i18n::text_with("common.and_more", &[("count", &more.to_string())])),
    }
}

#[cfg(test)]
mod tests;

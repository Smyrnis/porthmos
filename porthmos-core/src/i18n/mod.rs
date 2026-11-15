use std::borrow::Cow;

rust_i18n::i18n!("locales", fallback = "en");

const DEFAULT_LANGUAGE: &str = "en";
const LANGUAGE_VARIABLES: [&str; 3] = ["PORTHMOS_LANG", "LC_ALL", "LANG"];

pub fn language_of(setting: Option<&str>) -> String {
    let Some(setting) = setting.map(str::trim) else {
        return DEFAULT_LANGUAGE.to_string();
    };
    let language: String = setting.chars().take_while(|character| character.is_ascii_alphabetic()).collect();
    let rest = &setting[language.len()..];
    let well_formed = !language.is_empty() && (rest.is_empty() || rest.starts_with(['_', '-', '.', '@']));
    if !well_formed || matches!(language.as_str(), "C" | "POSIX") {
        return DEFAULT_LANGUAGE.to_string();
    }
    language.to_ascii_lowercase()
}

pub fn use_environment_language() {
    let setting = LANGUAGE_VARIABLES.iter().filter_map(|name| std::env::var(name).ok()).find(|value| !value.is_empty());
    set_language(&language_of(setting.as_deref()));
}

pub fn set_language(language: &str) {
    rust_i18n::set_locale(language);
}

pub fn language() -> String {
    rust_i18n::locale().to_string()
}

fn lookup<'a>(language: &str, key: &'a str) -> Option<Cow<'a, str>> {
    _rust_i18n_try_translate(language, key)
        .or_else(|| (language != DEFAULT_LANGUAGE).then(|| _rust_i18n_try_translate(DEFAULT_LANGUAGE, key)).flatten())
}

fn text_in(language: &str, key: &str) -> String {
    lookup(language, key).map_or_else(|| key.to_string(), Cow::into_owned)
}

pub fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut filled = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("%{") {
        filled.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let known = after
            .find('}')
            .and_then(|end| values.iter().find(|(name, _)| *name == &after[..end]).map(|(_, value)| (end, *value)));
        match known {
            Some((end, value)) => {
                filled.push_str(value);
                rest = &after[end + 1..];
            }
            None => {
                filled.push_str("%{");
                rest = after;
            }
        }
    }
    filled.push_str(rest);
    filled
}

fn plural_in(language: &str, key: &str, count: usize) -> String {
    let form = if count == 1 { "one" } else { "other" };
    fill(&text_in(language, &format!("{key}.{form}")), &[("count", &count.to_string())])
}

pub fn text(key: &str) -> String {
    text_in(&language(), key)
}

pub fn text_with(key: &str, values: &[(&str, &str)]) -> String {
    fill(&text(key), values)
}

pub fn plural(key: &str, count: usize) -> String {
    plural_in(&language(), key, count)
}

#[cfg(test)]
mod tests;

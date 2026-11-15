use std::borrow::Cow;

rust_i18n::i18n!("locales", fallback = "en");

const DEFAULT_LANGUAGE: &str = "en";
const AUTOMATIC: &str = "auto";
const LANGUAGE_VARIABLES: [&str; 3] = ["PORTHMOS_LANG", "LC_ALL", "LANG"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
}

const LANGUAGES: [Language; 2] = [Language { code: "en", name: "English" }, Language { code: "el", name: "Ελληνικά" }];

pub fn available_languages() -> &'static [Language] {
    &LANGUAGES
}

pub fn is_available(code: &str) -> bool {
    LANGUAGES.iter().any(|language| language.code == code)
}

pub fn is_automatic(setting: &str) -> bool {
    let setting = setting.trim();
    setting.is_empty() || setting.eq_ignore_ascii_case(AUTOMATIC)
}

pub fn chosen_language(setting: Option<&str>, environment: Option<&str>) -> String {
    if let Some(code) = setting.filter(|code| is_available(code)) {
        return code.to_string();
    }
    let from_environment = language_of(environment);
    if is_available(&from_environment) { from_environment } else { DEFAULT_LANGUAGE.to_string() }
}

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

fn environment_setting() -> Option<String> {
    LANGUAGE_VARIABLES.iter().filter_map(|name| std::env::var(name).ok()).find(|value| !value.is_empty())
}

pub fn use_environment_language() {
    use_language(None);
}

pub fn use_language(setting: Option<&str>) {
    set_language(&chosen_language(setting, environment_setting().as_deref()));
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

fn plural_with_in(language: &str, key: &str, count: usize, values: &[(&str, &str)]) -> String {
    fill(&plural_in(language, key, count), values)
}

pub fn label_key(english: &str) -> String {
    let mut slug = String::new();
    for character in english.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if !slug.ends_with('_') && !slug.is_empty() {
            slug.push('_');
        }
    }
    format!("label.{slug}")
}

fn label_in(language: &str, english: &str) -> String {
    let key = label_key(english);
    lookup(language, &key).map_or_else(|| english.to_string(), Cow::into_owned)
}

pub fn label(english: &str) -> String {
    label_in(&language(), english)
}

fn text_or_in(language: &str, key: &str, fallback: &str) -> String {
    lookup(language, key).map_or_else(|| fallback.to_string(), Cow::into_owned)
}

pub fn text_or(key: &str, fallback: &str) -> String {
    text_or_in(&language(), key, fallback)
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

pub fn plural_with(key: &str, count: usize, values: &[(&str, &str)]) -> String {
    plural_with_in(&language(), key, count, values)
}

#[cfg(test)]
mod tests;

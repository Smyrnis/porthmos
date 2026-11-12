use std::fs;
use std::path::Path;

use super::{ColorScheme, Rgb, ThemeBase};
use crate::{Paths, persist::toml_problem};

pub const FILE_NAME: &str = "theme.toml";

pub fn load(paths: &Paths, config_accent: Option<Rgb>) -> (ColorScheme, Vec<String>) {
    load_from(&paths.theme_file(), config_accent)
}

pub fn load_from(path: &Path, config_accent: Option<Rgb>) -> (ColorScheme, Vec<String>) {
    let fallback = ColorScheme::default().with_accent(config_accent);
    match fs::read_to_string(path) {
        Ok(text) => parse(&text, config_accent),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (fallback, Vec::new()),
        Err(err) => (fallback, vec![format!("could not read {FILE_NAME}: {err}")]),
    }
}

pub fn base_in(text: &str) -> ThemeBase {
    let Ok(table) = toml::from_str::<toml::Table>(text) else {
        return ThemeBase::default();
    };
    table.get("base").and_then(toml::Value::as_str).and_then(ThemeBase::parse).unwrap_or_default()
}

pub fn read_base(path: &Path) -> ThemeBase {
    fs::read_to_string(path).map(|text| base_in(&text)).unwrap_or_default()
}

pub fn parse(text: &str, config_accent: Option<Rgb>) -> (ColorScheme, Vec<String>) {
    let fallback = ColorScheme::default().with_accent(config_accent);
    let table: toml::Table = match toml::from_str(text) {
        Ok(table) => table,
        Err(err) => return (fallback, vec![format!("failed to parse {FILE_NAME}: {}", toml_problem(text, &err))]),
    };
    let mut warnings = Vec::new();
    let base = chosen_base(&table, &mut warnings);
    let mut scheme = base.scheme().with_accent(config_accent);
    for (key, value) in &table {
        match key.as_str() {
            "base" => {}
            "colors" => match value.as_table() {
                Some(colors) => apply_colors(&mut scheme, colors, &mut warnings),
                None => warnings.push(format!("[colors] in {FILE_NAME} must be a table; using the default colours")),
            },
            _ => warnings.push(format!("unknown {FILE_NAME} setting \"{key}\"")),
        }
    }
    (scheme, warnings)
}

fn chosen_base(table: &toml::Table, warnings: &mut Vec<String>) -> ThemeBase {
    let Some(value) = table.get("base") else {
        return ThemeBase::default();
    };
    let names = ThemeBase::NAMES.join(", ");
    match value.as_str() {
        Some(name) => ThemeBase::parse(name).unwrap_or_else(|| {
            warnings.push(format!("unknown base \"{name}\" in {FILE_NAME}: expected one of {names}"));
            ThemeBase::default()
        }),
        None => {
            warnings.push(format!("base in {FILE_NAME} must be a string, one of {names}; using the default"));
            ThemeBase::default()
        }
    }
}

fn apply_colors(scheme: &mut ColorScheme, colors: &toml::Table, warnings: &mut Vec<String>) {
    for (name, value) in colors {
        let Some(slot) = scheme.role_mut(name) else {
            warnings.push(format!("unknown colour \"{name}\" in {FILE_NAME}"));
            continue;
        };
        let Some(text) = value.as_str() else {
            warnings.push(format!("colour {name} in {FILE_NAME} must be a string; using the default"));
            continue;
        };
        match Rgb::parse(text) {
            Some(rgb) => *slot = rgb,
            None => warnings.push(format!(
                "invalid colour \"{text}\" for {name} in {FILE_NAME}: expected a colour like \"#9fb3c8\""
            )),
        }
    }
}

#[cfg(test)]
mod tests;

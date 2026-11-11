use std::fs;

use super::{ColorScheme, Rgb};
use crate::{Paths, persist::toml_problem};

pub const FILE_NAME: &str = "theme.toml";

pub fn load_onto(paths: &Paths, base: ColorScheme) -> (ColorScheme, Vec<String>) {
    match fs::read_to_string(paths.theme_file()) {
        Ok(text) => parse(base, &text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (base, Vec::new()),
        Err(err) => (base, vec![format!("could not read {FILE_NAME}: {err}")]),
    }
}

pub fn parse(base: ColorScheme, text: &str) -> (ColorScheme, Vec<String>) {
    let table: toml::Table = match toml::from_str(text) {
        Ok(table) => table,
        Err(err) => return (base, vec![format!("failed to parse {FILE_NAME}: {}", toml_problem(text, &err))]),
    };
    let mut scheme = base;
    let mut warnings = Vec::new();
    for (key, value) in &table {
        if key != "colors" {
            warnings.push(format!("unknown {FILE_NAME} setting \"{key}\""));
            continue;
        }
        match value.as_table() {
            Some(colors) => apply_colors(&mut scheme, colors, &mut warnings),
            None => warnings.push(format!("[colors] in {FILE_NAME} must be a table; using the default colours")),
        }
    }
    (scheme, warnings)
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

use std::fs;
use std::io;
use std::path::Path;

use porthmos_core::UpdateMode;
use toml_edit::{Array, DocumentMut, Item, Table, TableLike, Value};

const FILE_NAME: &str = "config.toml";
const NEW_FILE_MODE: u32 = 0o644;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    SetKey { action: String, chords: Vec<String> },
    ResetKey { action: String },
    SetEditor(Option<String>),
    SetAutoUpload(bool),
    SetUpdateMode(UpdateMode),
}

fn syntax_error(text: &str, error: &toml_edit::TomlError) -> String {
    let start = error.span().map_or(0, |span| span.start).min(text.len());
    let before = &text[..start];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |last| last.chars().count()) + 1;
    format!("{FILE_NAME} has a syntax error (line {line}, column {column}): {}", error.message())
}

fn section<'a>(root: &'a mut Table, path: &[&str], create: bool) -> Result<Option<&'a mut dyn TableLike>, String> {
    let mut current: &mut dyn TableLike = root;
    let mut trail = String::new();
    for (depth, name) in path.iter().enumerate() {
        if !trail.is_empty() {
            trail.push('.');
        }
        trail.push_str(name);
        if current.get(name).is_none() {
            if !create {
                return Ok(None);
            }
            let mut fresh = Table::new();
            fresh.set_implicit(depth + 1 < path.len());
            current.insert(name, Item::Table(fresh));
        }
        let next = current.get_mut(name).and_then(Item::as_table_like_mut);
        current = next.ok_or_else(|| format!("[{trail}] is not a table"))?;
    }
    Ok(Some(current))
}

fn put(table: &mut dyn TableLike, key: &str, value: Value) {
    match table.get_mut(key) {
        Some(existing) => {
            let decor = existing.as_value().map(|old| old.decor().clone());
            *existing = Item::Value(value);
            if let (Some(decor), Some(new)) = (decor, existing.as_value_mut()) {
                *new.decor_mut() = decor;
            }
        }
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}

fn chord_value(chords: &[String]) -> Value {
    match chords {
        [only] => Value::from(only.as_str()),
        many => Value::Array(many.iter().map(String::as_str).collect::<Array>()),
    }
}

fn mode_text(mode: UpdateMode) -> &'static str {
    match mode {
        UpdateMode::Auto => "auto",
        UpdateMode::Notify => "notify",
        UpdateMode::Off => "off",
    }
}

fn set_in(document: &mut DocumentMut, path: &[&str], key: &str, value: Value) -> Result<(), String> {
    let table = section(document.as_table_mut(), path, true)?
        .ok_or_else(|| format!("could not create [{}] in {FILE_NAME}", path.join(".")))?;
    put(table, key, value);
    Ok(())
}

fn remove_in(document: &mut DocumentMut, path: &[&str], key: &str) -> Result<(), String> {
    let removed = match section(document.as_table_mut(), path, false)? {
        Some(table) => table.remove(key).is_some(),
        None => false,
    };
    if removed {
        prune(document, path)?;
    }
    Ok(())
}

fn prune(document: &mut DocumentMut, path: &[&str]) -> Result<(), String> {
    for depth in (1..=path.len()).rev() {
        let (parent_path, name) = (&path[..depth - 1], path[depth - 1]);
        let Some(parent) = section(document.as_table_mut(), parent_path, false)? else {
            return Ok(());
        };
        let empty = parent.get(name).and_then(Item::as_table_like).is_some_and(|table| table.is_empty());
        if !empty {
            return Ok(());
        }
        parent.remove(name);
    }
    Ok(())
}

pub fn apply(text: &str, edit: &Edit) -> Result<String, String> {
    let mut document: DocumentMut = text.parse().map_err(|error| syntax_error(text, &error))?;
    match edit {
        Edit::SetKey { action, chords } => {
            if chords.is_empty() {
                return Err("a shortcut needs at least one key".to_string());
            }
            set_in(&mut document, &["gui", "keys"], action, chord_value(chords))?;
        }
        Edit::ResetKey { action } => remove_in(&mut document, &["gui", "keys"], action)?,
        Edit::SetEditor(editor) => match editor.as_deref().map(str::trim).filter(|editor| !editor.is_empty()) {
            Some(editor) => set_in(&mut document, &["edit"], "editor", Value::from(editor))?,
            None => remove_in(&mut document, &["edit"], "editor")?,
        },
        Edit::SetAutoUpload(on) => set_in(&mut document, &["edit"], "auto_upload", Value::from(*on))?,
        Edit::SetUpdateMode(mode) => set_in(&mut document, &["update"], "mode", Value::from(mode_text(*mode)))?,
    }
    Ok(document.to_string())
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).map_or(NEW_FILE_MODE, |metadata| metadata.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn mode_of(_path: &Path) -> u32 {
    NEW_FILE_MODE
}

pub fn save(path: &Path, edit: &Edit) -> Result<(), String> {
    let current = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };
    let updated = apply(&current, edit)?;
    if updated == current {
        return Ok(());
    }
    if let Some(folder) = path.parent().filter(|folder| !folder.as_os_str().is_empty()) {
        fs::create_dir_all(folder).map_err(|error| format!("could not create {}: {error}", folder.display()))?;
    }
    porthmos_core::persist::write_atomic(path, updated.as_bytes(), mode_of(path))
        .map_err(|error| format!("could not write {}: {error:#}", path.display()))
}

#[cfg(test)]
mod tests;

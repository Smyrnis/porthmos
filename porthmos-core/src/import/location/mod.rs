use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use porthmos_vfs::Environment;

use super::ImportSource;

fn non_empty(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn putty_session_folders(home: Option<&Path>, sessions: Option<OsString>, xdg: Option<OsString>) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    folders.extend(non_empty(sessions));
    folders.extend(non_empty(xdg).map(|xdg| xdg.join("putty").join("sessions")));
    if let Some(home) = home {
        folders.push(home.join(".config").join("putty").join("sessions"));
        folders.push(home.join(".putty").join("sessions"));
    }
    folders
}

fn candidates(source: ImportSource, env: &Environment, app_data: Option<&Path>) -> Vec<PathBuf> {
    match source {
        ImportSource::FileZilla => {
            let mut found: Vec<PathBuf> =
                env.home.iter().map(|home| home.join(".config").join("filezilla").join("sitemanager.xml")).collect();
            found.extend(app_data.map(|data| data.join("FileZilla").join("sitemanager.xml")));
            found
        }
        ImportSource::WinScp => app_data.map(|data| data.join("WinSCP.ini")).into_iter().collect(),
        ImportSource::Putty => putty_session_folders(
            env.home.as_deref(),
            std::env::var_os("PUTTYSESSIONS"),
            std::env::var_os("XDG_CONFIG_HOME"),
        ),
    }
}

pub fn default_location(source: ImportSource, env: &Environment, app_data: Option<&Path>) -> Option<PathBuf> {
    candidates(source, env, app_data).into_iter().find(|path| match source {
        ImportSource::Putty => path.is_dir(),
        ImportSource::FileZilla | ImportSource::WinScp => path.is_file(),
    })
}

pub fn path_from_text(text: &str, home: Option<&Path>) -> PathBuf {
    let trimmed = text.trim();
    match (trimmed.strip_prefix('~'), home) {
        (Some(""), Some(home)) => home.to_path_buf(),
        (Some(rest), Some(home)) if rest.starts_with('/') || rest.starts_with('\\') => home.join(&rest[1..]),
        _ => PathBuf::from(trimmed),
    }
}

pub fn default_location_here(source: ImportSource) -> Option<PathBuf> {
    let app_data = non_empty(std::env::var_os("APPDATA"));
    default_location(source, &Environment::from_process(), app_data.as_deref())
}

#[cfg(test)]
mod tests;

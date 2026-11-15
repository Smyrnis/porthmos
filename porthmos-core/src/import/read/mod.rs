use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use porthmos_vfs::Protocol;

use super::{
    ImportError, ImportPlan, ImportRow, ImportSource, MAX_INPUT_BYTES, Verdict, clean, escape, filezilla,
    finish::finish, putty, winscp,
};

const MAX_SESSION_FILES: usize = 10_000;
const MAX_SESSION_FILE_BYTES: usize = 1024 * 1024;
const NOT_UTF8: &str = "This is not a UTF-8 text file";
const TOO_MANY_FILES: &str = "This folder has too many files to be PuTTY sessions";
const NO_SESSIONS: &str = "There are no PuTTY sessions in this folder";
const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

fn unreadable(error: io::Error) -> ImportError {
    let cause = match error.kind() {
        io::ErrorKind::NotFound => "no such file or folder".to_string(),
        io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => error.to_string(),
    };
    ImportError::Unreadable(cause)
}

fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, ImportError> {
    let file = File::open(path).map_err(unreadable)?;
    let metadata = file.metadata().map_err(unreadable)?;
    if !metadata.is_file() {
        return Err(ImportError::Unreadable("it is not a file".to_string()));
    }
    if metadata.len() > limit as u64 {
        return Err(ImportError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(unreadable)?;
    if bytes.len() > limit { Err(ImportError::TooLarge) } else { Ok(bytes) }
}

fn read_text(path: &Path) -> Result<String, ImportError> {
    let bytes = read_limited(path, MAX_INPUT_BYTES)?;
    let body = bytes.strip_prefix(&UTF8_BOM).unwrap_or(&bytes);
    String::from_utf8(body.to_vec()).map_err(|_| ImportError::WrongFormat(NOT_UTF8))
}

fn read_putty_folder(folder: &Path) -> Result<Vec<ImportRow>, ImportError> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(folder).map_err(unreadable)?.flatten().take(MAX_SESSION_FILES + 1) {
        if files.len() == MAX_SESSION_FILES {
            return Err(ImportError::WrongFormat(TOO_MANY_FILES));
        }
        let path = entry.path();
        if fs::metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
            files.push((entry.file_name().to_string_lossy().into_owned(), path));
        }
    }
    files.sort();

    let mut rows = Vec::new();
    let mut total = 0usize;
    for (file_name, path) in files {
        match read_limited(&path, MAX_SESSION_FILE_BYTES) {
            Ok(bytes) => {
                total += bytes.len();
                if total > MAX_INPUT_BYTES {
                    return Err(ImportError::TooLarge);
                }
                rows.extend(putty::parse_session(&file_name, &String::from_utf8_lossy(&bytes)));
            }
            Err(_) => {
                let name = clean::name(&escape::decode(&file_name)).unwrap_or_else(|| "(unnamed)".to_string());
                rows.push(ImportRow {
                    name,
                    candidate: None,
                    verdict: Verdict::Invalid("could not be read".to_string()),
                });
            }
        }
    }
    if rows.is_empty() {
        return Err(ImportError::WrongFormat(NO_SESSIONS));
    }
    Ok(rows)
}

pub fn read_plan(
    import_id: u64, source: ImportSource, path: &Path, protocols: &[Arc<dyn Protocol>], taken_names: &HashSet<String>,
) -> Result<ImportPlan, ImportError> {
    let rows = match source {
        ImportSource::FileZilla => filezilla::parse(&read_text(path)?)?,
        ImportSource::WinScp => winscp::parse(&read_text(path)?)?,
        ImportSource::Putty if path.is_dir() => read_putty_folder(path)?,
        ImportSource::Putty => putty::parse_reg(&putty::decode_reg_text(&read_limited(path, MAX_INPUT_BYTES)?)?)?,
    };
    Ok(ImportPlan { import_id, source, rows: finish(rows, protocols, taken_names) })
}

#[cfg(test)]
mod tests;

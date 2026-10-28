use std::{
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use super::InstallError;
use super::io_failure;
#[cfg(windows)]
use super::{check_time, copy_with_deadline, old_path_of};
#[cfg(windows)]
use crate::Staged;

const BUSY_WAIT: Duration = Duration::from_millis(20);
const ROLLBACK_PATIENCE: Duration = Duration::from_secs(2);
const SHARING_VIOLATION: i32 = 32;
const LOCK_VIOLATION: i32 = 33;
const VERBATIM_PREFIX: &str = r"\\?\";
const MANAGED_FOLDERS: [(&str, &str); 6] = [
    ("program files", "Program Files"),
    ("program files (x86)", "Program Files (x86)"),
    ("windowsapps", "WindowsApps"),
    ("scoop", "Scoop"),
    ("winget", "WinGet"),
    ("chocolatey", "Chocolatey"),
];

fn normalised(text: &str) -> String {
    text.strip_prefix(VERBATIM_PREFIX).unwrap_or(text).replace('/', "\\").to_lowercase()
}

pub(super) fn managed_reason(path_text: &str, home_text: Option<&str>) -> Option<String> {
    let path = normalised(path_text);
    if let Some((_, label)) = MANAGED_FOLDERS.iter().find(|(folder, _)| path.contains(&format!("\\{folder}\\"))) {
        return Some(format!("it is installed under {label}, which a package manager looks after"));
    }
    let cargo_bin = format!("{}\\.cargo\\bin\\", normalised(home_text?).trim_end_matches('\\'));
    path.starts_with(&cargo_bin)
        .then(|| "it was installed with cargo into .cargo\\bin; update it with cargo install".to_string())
}

type Rename<'a> = &'a dyn Fn(&Path, &Path) -> io::Result<()>;

fn is_busy(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
        || matches!(error.raw_os_error(), Some(SHARING_VIOLATION | LOCK_VIOLATION))
}

fn rename_when_not_busy(
    rename: Rename, from: &Path, to: &Path, wait: Duration, deadline: Instant,
) -> Result<(), InstallError> {
    loop {
        match rename(from, to) {
            Err(error) if is_busy(&error) => {
                if Instant::now() >= deadline {
                    return Err(InstallError::TimedOut);
                }
                thread::sleep(wait);
            }
            other => return other.map_err(io_failure),
        }
    }
}

fn replace_by_renaming_with(
    pending: &Path, real: &Path, old: &Path, rename: Rename, wait: Duration, deadline: Instant,
) -> Result<(), InstallError> {
    if let Err(error) = fs::remove_file(old)
        && error.kind() != io::ErrorKind::NotFound
    {
        return Err(InstallError::Io(format!(
            "the previous version {} can't be removed ({error}); close any other Porthmos window and quit again",
            old.display()
        )));
    }
    rename_when_not_busy(rename, real, old, wait, deadline)?;
    let Err(failure) = rename_when_not_busy(rename, pending, real, wait, deadline) else { return Ok(()) };
    let rollback_deadline = Instant::now() + ROLLBACK_PATIENCE;
    if rename_when_not_busy(rename, old, real, wait, rollback_deadline).is_err() {
        return Err(InstallError::Io(format!(
            "the update could not be installed and the previous version could not be put back; it is saved as {}",
            old.display()
        )));
    }
    Err(failure)
}

pub(super) fn replace_by_renaming(
    pending: &Path, real: &Path, old: &Path, deadline: Instant,
) -> Result<(), InstallError> {
    replace_by_renaming_with(pending, real, old, &|from, to| fs::rename(from, to), BUSY_WAIT, deadline)
}

#[cfg(windows)]
pub(super) fn replace_executable(
    staged: &Staged, real: &Path, pending: &Path, deadline: Instant,
) -> Result<PathBuf, InstallError> {
    copy_with_deadline(&staged.binary, pending, 0o755, deadline)?;
    check_time(deadline)?;
    let old = old_path_of(real);
    check_time(deadline)?;
    replace_by_renaming(pending, real, &old, deadline)?;
    Ok(old)
}

#[cfg(test)]
mod tests;

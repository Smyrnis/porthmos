use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Instant,
};

use super::{InstallError, check_time, copy_with_deadline, io_failure, old_path_of};
use crate::Staged;

fn keep_previous_version(real: &Path, old: &Path) -> Result<(), InstallError> {
    let folder = real.parent().unwrap_or_else(|| Path::new("."));
    let pending = folder.join(format!(".porthmos.update.old.{}", std::process::id()));
    let _ = fs::remove_file(&pending);
    let made = fs::hard_link(real, &pending).or_else(|_| fs::copy(real, &pending).map(|_| ()));
    let result = made.and_then(|()| fs::rename(&pending, old)).map_err(io_failure);
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}

pub(super) fn replace_executable(
    staged: &Staged, real: &Path, pending: &Path, deadline: Instant,
) -> Result<PathBuf, InstallError> {
    let mode = fs::metadata(real).map(|meta| meta.permissions().mode() & 0o7777).unwrap_or(0o755);
    copy_with_deadline(&staged.binary, pending, mode, deadline)?;
    check_time(deadline)?;
    let old = old_path_of(real);
    keep_previous_version(real, &old)?;
    check_time(deadline)?;
    fs::rename(pending, real).map_err(io_failure)?;
    Ok(old)
}

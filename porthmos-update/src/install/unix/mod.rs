use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use semver::Version;

use super::{InstallError, Installed, managed_reason, old_path_of};
use crate::{Staged, UpdateDir, discard_staged, sha256_hex_of_file, unpack::version_output};

const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

fn check_time(deadline: Instant) -> Result<(), InstallError> {
    if Instant::now() >= deadline { Err(InstallError::TimedOut) } else { Ok(()) }
}

fn io_failure(error: io::Error) -> InstallError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        InstallError::NotWritable(error.to_string())
    } else {
        InstallError::Io(error.to_string())
    }
}

fn version_on_disk(executable: &Path) -> Option<Version> {
    let output = version_output(executable, PROBE_TIMEOUT).ok()?;
    output.strip_prefix("porthmos ")?.trim().parse().ok()
}

fn copy_with_deadline(from: &Path, to: &Path, mode: u32, deadline: Instant) -> Result<(), InstallError> {
    let mut source = fs::File::open(from).map_err(io_failure)?;
    let mut target = fs::OpenOptions::new().write(true).create(true).truncate(true).open(to).map_err(io_failure)?;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        check_time(deadline)?;
        let read = source.read(&mut buffer).map_err(io_failure)?;
        if read == 0 {
            break;
        }
        target.write_all(&buffer[..read]).map_err(io_failure)?;
    }
    target.set_permissions(fs::Permissions::from_mode(mode)).map_err(io_failure)?;
    target.sync_all().map_err(io_failure)
}

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

fn replace_executable(
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

pub(super) fn install(
    dir: &UpdateDir, staged: &Staged, executable: &Path, budget: Duration, home: Option<&Path>,
) -> Result<Installed, InstallError> {
    let deadline = Instant::now() + budget;
    check_time(deadline)?;
    let real = fs::canonicalize(executable).map_err(io_failure)?;
    if let Some(reason) = managed_reason(&real, home) {
        return Err(InstallError::Managed(reason));
    }
    let on_disk = fs::metadata(&staged.binary).ok().filter(|meta| meta.is_file());
    let Some(on_disk) = on_disk else { return Err(InstallError::NothingStaged) };
    let intact =
        on_disk.len() == staged.size && sha256_hex_of_file(&staged.binary).is_ok_and(|hash| hash == staged.sha256);
    if !intact {
        discard_staged(dir);
        return Err(InstallError::Corrupt);
    }
    check_time(deadline)?;
    if version_on_disk(executable).is_some_and(|current| current >= staged.version) {
        discard_staged(dir);
        return Err(InstallError::AlreadyCurrent);
    }
    check_time(deadline)?;

    let folder = real.parent().ok_or_else(|| InstallError::Io("the program has no folder".to_string()))?;
    let pending = folder.join(format!(".porthmos.update.{}", std::process::id()));
    let outcome = replace_executable(staged, &real, &pending, deadline);
    if outcome.is_err() {
        let _ = fs::remove_file(&pending);
    }
    let previous = outcome?;
    discard_staged(dir);
    Ok(Installed { version: staged.version.clone(), previous })
}

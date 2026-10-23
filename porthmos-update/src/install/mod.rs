use std::{
    ffi::OsString,
    fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use semver::Version;

use crate::{Staged, UpdateDir, discard_staged, sha256_hex_of_file, unpack::version_output};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallPlace {
    Updatable { executable: PathBuf },
    Managed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub version: Version,
    pub previous: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    NothingStaged,
    Corrupt,
    AlreadyCurrent,
    NotWritable(String),
    TimedOut,
    Unsupported,
    Io(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::NothingStaged => f.write_str("there is no staged update to install"),
            InstallError::Corrupt => f.write_str("the staged update changed on disk and was discarded"),
            InstallError::AlreadyCurrent => f.write_str("the installed program is already up to date"),
            InstallError::NotWritable(text) => write!(f, "can't write to the install folder: {text}"),
            InstallError::TimedOut => f.write_str("installing the update took too long"),
            InstallError::Unsupported => f.write_str("installing updates is not supported on this system yet"),
            InstallError::Io(text) => write!(f, "couldn't install the update: {text}"),
        }
    }
}

impl std::error::Error for InstallError {}

const MANAGED_PREFIXES: [&str; 5] = ["/usr", "/opt/homebrew", "/home/linuxbrew", "/nix/store", "/snap"];
const OLD_SUFFIX: &str = ".old";
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

fn managed_reason(real_path: &Path, home: Option<&Path>) -> Option<String> {
    if let Some(prefix) = MANAGED_PREFIXES.iter().find(|prefix| real_path.starts_with(prefix)) {
        return Some(format!("it is installed under {prefix}, which a package manager looks after"));
    }
    let cargo_bin = home?.join(".cargo").join("bin");
    real_path
        .starts_with(&cargo_bin)
        .then(|| "it was installed with cargo into .cargo/bin; update it with cargo install".to_string())
}

fn can_create_files_in(folder: &Path) -> bool {
    let probe = folder.join(format!(".porthmos.write-test.{}", std::process::id()));
    let created = fs::OpenOptions::new().write(true).create_new(true).open(&probe).is_ok();
    let _ = fs::remove_file(&probe);
    created
}

pub fn install_place(current_exe: &Path, home: Option<&Path>) -> InstallPlace {
    let Ok(real) = fs::canonicalize(current_exe) else {
        return InstallPlace::Managed { reason: "its own file could not be found".to_string() };
    };
    if let Some(reason) = managed_reason(&real, home) {
        return InstallPlace::Managed { reason };
    }
    match real.parent() {
        Some(folder) if can_create_files_in(folder) => InstallPlace::Updatable { executable: real },
        Some(folder) => InstallPlace::Managed { reason: format!("{} isn't writable", folder.display()) },
        None => InstallPlace::Managed { reason: "it has no folder".to_string() },
    }
}

fn old_path_of(real: &Path) -> PathBuf {
    let mut name = real.file_name().map(OsString::from).unwrap_or_default();
    name.push(OLD_SUFFIX);
    real.with_file_name(name)
}

pub fn forget_previous_version(executable: &Path) {
    let real = fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
    let _ = fs::remove_file(old_path_of(&real));
}

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

#[cfg(unix)]
fn copy_with_deadline(from: &Path, to: &Path, mode: u32, deadline: Instant) -> Result<(), InstallError> {
    use std::os::unix::fs::PermissionsExt;
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

#[cfg(unix)]
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

#[cfg(unix)]
fn replace_executable(
    staged: &Staged, real: &Path, pending: &Path, deadline: Instant,
) -> Result<PathBuf, InstallError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(real).map(|meta| meta.permissions().mode() & 0o7777).unwrap_or(0o755);
    copy_with_deadline(&staged.binary, pending, mode, deadline)?;
    check_time(deadline)?;
    let old = old_path_of(real);
    keep_previous_version(real, &old)?;
    check_time(deadline)?;
    fs::rename(pending, real).map_err(io_failure)?;
    Ok(old)
}

#[cfg(unix)]
pub fn install(
    dir: &UpdateDir, staged: &Staged, executable: &Path, budget: Duration,
) -> Result<Installed, InstallError> {
    let deadline = Instant::now() + budget;
    check_time(deadline)?;
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

    let real = fs::canonicalize(executable).map_err(io_failure)?;
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

#[cfg(not(unix))]
pub fn install(
    _dir: &UpdateDir, _staged: &Staged, _executable: &Path, _budget: Duration,
) -> Result<Installed, InstallError> {
    Err(InstallError::Unsupported)
}

#[cfg(test)]
mod tests;

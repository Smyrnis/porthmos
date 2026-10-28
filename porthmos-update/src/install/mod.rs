use std::{
    ffi::OsString,
    fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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
    Managed(String),
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
            InstallError::Managed(reason) => write!(f, "the program is managed by something else: {reason}"),
            InstallError::TimedOut => f.write_str("installing the update took too long"),
            InstallError::Unsupported => f.write_str("installing updates is not supported on this system yet"),
            InstallError::Io(text) => write!(f, "couldn't install the update: {text}"),
        }
    }
}

impl std::error::Error for InstallError {}

#[cfg(not(windows))]
const MANAGED_PREFIXES: [&str; 5] = ["/usr", "/opt/homebrew", "/home/linuxbrew", "/nix/store", "/snap"];
const OLD_SUFFIX: &str = ".old";
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

#[cfg(windows)]
fn managed_reason(real_path: &Path, home: Option<&Path>) -> Option<String> {
    let home = home.map(|home| fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf()));
    windows::managed_reason(&real_path.to_string_lossy(), home.as_ref().map(|home| home.to_string_lossy()).as_deref())
}

#[cfg(not(windows))]
fn managed_reason(real_path: &Path, home: Option<&Path>) -> Option<String> {
    if let Some(prefix) = MANAGED_PREFIXES.iter().find(|prefix| real_path.starts_with(prefix)) {
        return Some(format!("it is installed under {prefix}, which a package manager looks after"));
    }
    let cargo_bin = home?.join(".cargo").join("bin");
    real_path
        .starts_with(&cargo_bin)
        .then(|| "it was installed with cargo into .cargo/bin; update it with cargo install".to_string())
}

static PROBES: AtomicU64 = AtomicU64::new(0);

fn can_create_files_in(folder: &Path) -> bool {
    let probe =
        folder.join(format!(".porthmos.write-test.{}.{}", std::process::id(), PROBES.fetch_add(1, Ordering::Relaxed)));
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

#[cfg_attr(not(unix), allow(unused_variables))]
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        target.set_permissions(fs::Permissions::from_mode(mode)).map_err(io_failure)?;
    }
    target.sync_all().map_err(io_failure)
}

#[cfg(any(unix, windows))]
type Replace = fn(&Staged, &Path, &Path, Instant) -> Result<PathBuf, InstallError>;

#[cfg(any(unix, windows))]
fn run_install(
    dir: &UpdateDir, staged: &Staged, executable: &Path, budget: Duration, home: Option<&Path>, replace: Replace,
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
    let outcome = replace(staged, &real, &pending, deadline);
    if outcome.is_err() {
        let _ = fs::remove_file(&pending);
    }
    let previous = outcome?;
    discard_staged(dir);
    Ok(Installed { version: staged.version.clone(), previous })
}

#[cfg(unix)]
mod unix;
#[cfg(any(windows, test))]
mod windows;

#[cfg(unix)]
use unix::replace_executable;
#[cfg(windows)]
use windows::replace_executable;

#[cfg(any(unix, windows))]
pub fn install(
    dir: &UpdateDir, staged: &Staged, executable: &Path, budget: Duration,
) -> Result<Installed, InstallError> {
    install_for(dir, staged, executable, budget, std::env::home_dir().as_deref())
}

#[cfg(any(unix, windows))]
fn install_for(
    dir: &UpdateDir, staged: &Staged, executable: &Path, budget: Duration, home: Option<&Path>,
) -> Result<Installed, InstallError> {
    run_install(dir, staged, executable, budget, home, replace_executable)
}

#[cfg(not(any(unix, windows)))]
pub fn install(
    _dir: &UpdateDir, _staged: &Staged, _executable: &Path, _budget: Duration,
) -> Result<Installed, InstallError> {
    Err(InstallError::Unsupported)
}

#[cfg(all(test, unix))]
mod tests;

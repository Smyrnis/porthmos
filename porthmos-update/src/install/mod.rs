use std::{
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use semver::Version;

use crate::{Staged, UpdateDir};

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

const MANAGED_PREFIXES: [&str; 5] = ["/usr", "/opt/homebrew", "/home/linuxbrew", "/nix/store", "/snap"];
const OLD_SUFFIX: &str = ".old";

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

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub fn install(
    dir: &UpdateDir, staged: &Staged, executable: &Path, budget: Duration,
) -> Result<Installed, InstallError> {
    install_for(dir, staged, executable, budget, std::env::home_dir().as_deref())
}

#[cfg(unix)]
fn install_for(
    dir: &UpdateDir, staged: &Staged, executable: &Path, budget: Duration, home: Option<&Path>,
) -> Result<Installed, InstallError> {
    unix::install(dir, staged, executable, budget, home)
}

#[cfg(not(unix))]
pub fn install(
    _dir: &UpdateDir, _staged: &Staged, _executable: &Path, _budget: Duration,
) -> Result<Installed, InstallError> {
    Err(InstallError::Unsupported)
}

#[cfg(all(test, unix))]
mod tests;

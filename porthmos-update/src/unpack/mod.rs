use std::{
    fs::{self, File},
    io::{self, Read, Seek},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use semver::Version;

const BINARY_NAME: &str = "porthmos";
#[cfg(windows)]
pub(crate) const PROGRAM_FILE: &str = "porthmos.exe";
#[cfg(not(windows))]
pub(crate) const PROGRAM_FILE: &str = "porthmos";
const ARCHIVE_SLACK: u64 = 16 * 1024 * 1024;
const OUTPUT_LIMIT: u64 = 4096;
const POLL: Duration = Duration::from_millis(10);
const BUSY_RETRY: Duration = Duration::from_millis(20);
const BUSY_RETRIES: usize = 100;

fn spawn_when_not_busy(command: &mut Command) -> io::Result<std::process::Child> {
    let mut attempts = 0;
    loop {
        match command.spawn() {
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && attempts < BUSY_RETRIES => {
                attempts += 1;
                thread::sleep(BUSY_RETRY);
            }
            other => return other,
        }
    }
}

fn copy_member(entry: &mut impl Read, destination: &Path, max_bytes: u64) -> Result<()> {
    let mut file = File::create(destination).with_context(|| format!("couldn't create {}", destination.display()))?;
    let copied = io::copy(&mut entry.take(max_bytes + 1), &mut file)?;
    if copied > max_bytes {
        bail!("the program in the archive is larger than {max_bytes} bytes");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o755))?;
    }
    file.sync_all()?;
    Ok(())
}

fn copy_from_tar(archive: File, destination: &Path, max_bytes: u64, member: &str) -> Result<()> {
    let bounded = GzDecoder::new(archive).take(max_bytes.saturating_add(ARCHIVE_SLACK));
    let mut tar = tar::Archive::new(bounded);
    for entry in tar.entries().context("the archive is not readable")? {
        let mut entry = entry.context("the archive is damaged")?;
        let is_the_program = entry.header().entry_type() == tar::EntryType::Regular
            && entry.path().is_ok_and(|path| path == Path::new(member));
        if is_the_program {
            return copy_member(&mut entry, destination, max_bytes);
        }
    }
    bail!("the archive does not contain the {member} program")
}

fn copy_from_zip(archive: File, destination: &Path, max_bytes: u64, member: &str) -> Result<()> {
    let mut zip = zip::ZipArchive::new(archive).context("the archive is not readable")?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).context("the archive is damaged")?;
        let is_the_program = entry.is_file() && entry.enclosed_name().is_some_and(|path| path == Path::new(member));
        if is_the_program {
            if entry.size() > max_bytes {
                bail!("the program in the archive is larger than {max_bytes} bytes");
            }
            return copy_member(&mut entry, destination, max_bytes);
        }
    }
    bail!("the archive does not contain the {member} program")
}

fn find_and_copy(archive: &Path, destination: &Path, max_bytes: u64, member: &str) -> Result<()> {
    let mut file = File::open(archive).with_context(|| format!("couldn't open {}", archive.display()))?;
    let mut magic = [0u8; 4];
    let read = file.read(&mut magic)?;
    file.rewind()?;
    match &magic[..read] {
        [0x1f, 0x8b, ..] => copy_from_tar(file, destination, max_bytes, member),
        [0x50, 0x4b, 0x03, 0x04] => copy_from_zip(file, destination, max_bytes, member),
        _ => bail!("the archive is not a .tar.gz or .zip file"),
    }
}

pub(crate) fn extract_binary_named(archive: &Path, destination: &Path, max_bytes: u64, member: &str) -> Result<()> {
    let result = find_and_copy(archive, destination, max_bytes, member);
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

pub(crate) fn extract_binary(archive: &Path, destination: &Path, max_bytes: u64) -> Result<()> {
    extract_binary_named(archive, destination, max_bytes, PROGRAM_FILE)
}

pub(crate) fn version_output(binary: &Path, timeout: Duration) -> Result<String> {
    let mut command = Command::new(binary);
    command.arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    let mut child = spawn_when_not_busy(&mut command).with_context(|| format!("couldn't run {}", binary.display()))?;
    let stdout = child.stdout.take().context("no output pipe")?;
    let reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.take(OUTPUT_LIMIT).read_to_string(&mut text);
        text
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("the program did not answer within {} seconds", timeout.as_secs().max(1));
        }
        thread::sleep(POLL);
    };
    let output = reader.join().unwrap_or_default();
    if !status.success() {
        bail!("the program exited with {status}");
    }
    Ok(output.trim().to_string())
}

pub(crate) fn health_check(binary: &Path, expected: &Version, timeout: Duration) -> Result<()> {
    let output = version_output(binary, timeout)?;
    if output != format!("{BINARY_NAME} {expected}") {
        let shown: String = output.chars().take(80).collect();
        bail!("the program reported \"{shown}\" instead of {BINARY_NAME} {expected}");
    }
    Ok(())
}

#[cfg(test)]
mod tests;

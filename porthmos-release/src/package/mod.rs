use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use flate2::{Compression, write::GzEncoder};
use porthmos_update::sha256_hex;
use semver::Version;

const BINARY_NAME: &str = "porthmos";
const WINDOWS_BINARY_NAME: &str = "porthmos.exe";
const LICENSE_NAME: &str = "LICENSE";
const EXECUTABLE_MODE: u32 = 0o755;
const FILE_MODE: u32 = 0o644;

fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("couldn't read {}", path.display()))
}

fn append(builder: &mut tar::Builder<GzEncoder<Vec<u8>>>, name: &str, bytes: &[u8], mode: u32) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    builder.append_data(&mut header, name, bytes)?;
    Ok(())
}

fn archive_of(binary: &[u8], license: &[u8]) -> Result<Vec<u8>> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    append(&mut builder, BINARY_NAME, binary, EXECUTABLE_MODE)?;
    append(&mut builder, LICENSE_NAME, license, FILE_MODE)?;
    Ok(builder.into_inner()?.finish()?)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut temporary = PathBuf::from(path);
    temporary.as_mut_os_string().push(".tmp");
    let written = fs::File::create(&temporary).and_then(|mut file| file.write_all(bytes));
    let result = written.and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("couldn't write {}", path.display()))
}

fn zip_of(binary: &[u8], license: &[u8]) -> Result<Vec<u8>> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(EXECUTABLE_MODE);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer.start_file(WINDOWS_BINARY_NAME, options)?;
    writer.write_all(binary)?;
    writer.start_file(LICENSE_NAME, options.unix_permissions(FILE_MODE))?;
    writer.write_all(license)?;
    Ok(writer.finish()?.into_inner())
}

pub fn package(target: &str, version: &Version, binary: &Path, license: &Path, out: &Path) -> Result<()> {
    let (binary, license) = (read(binary)?, read(license)?);
    let (archive, extension) = if target.contains("windows") {
        (zip_of(&binary, &license)?, "zip")
    } else {
        (archive_of(&binary, &license)?, "tar.gz")
    };
    let plain_name = format!("{BINARY_NAME}-{target}.{extension}");
    let checksum = format!("{}  {plain_name}\n", sha256_hex(&archive));

    fs::create_dir_all(out).with_context(|| format!("couldn't create {}", out.display()))?;
    write_atomically(&out.join(format!("{BINARY_NAME}-{version}-{target}.{extension}")), &archive)?;
    write_atomically(&out.join(&plain_name), &archive)?;
    write_atomically(&out.join(format!("{plain_name}.sha256")), checksum.as_bytes())
}

#[cfg(test)]
mod tests;

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use flate2::read::GzDecoder;
use porthmos_update::sha256_hex;

use super::*;

const BINARY: &[u8] = b"#!/bin/sh\necho porthmos-stub\n";
const LICENSE: &[u8] = b"MIT License\n";
const TARGET: &str = "x86_64-unknown-linux-gnu";

struct Fixture {
    directory: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("porthmos"), BINARY).unwrap();
        std::fs::write(directory.path().join("LICENSE"), LICENSE).unwrap();
        std::fs::create_dir(directory.path().join("dist")).unwrap();
        Self { directory }
    }

    fn binary(&self) -> PathBuf {
        self.directory.path().join("porthmos")
    }

    fn license(&self) -> PathBuf {
        self.directory.path().join("LICENSE")
    }

    fn out(&self) -> PathBuf {
        self.directory.path().join("dist")
    }

    fn run(&self) -> Result<()> {
        package(TARGET, &Version::new(0, 4, 0), &self.binary(), &self.license(), &self.out())
    }
}

fn entries_of(archive: &Path) -> Vec<(String, u32, Vec<u8>)> {
    let mut archive = tar::Archive::new(GzDecoder::new(File::open(archive).unwrap()));
    archive
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_string_lossy().into_owned();
            let mode = entry.header().mode().unwrap();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            (name, mode, bytes)
        })
        .collect()
}

#[test]
fn the_versioned_and_version_less_archives_are_identical() {
    let fixture = Fixture::new();

    fixture.run().unwrap();

    let versioned = std::fs::read(fixture.out().join("porthmos-0.4.0-x86_64-unknown-linux-gnu.tar.gz")).unwrap();
    let plain = std::fs::read(fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz")).unwrap();
    assert_eq!(versioned, plain);
}

#[test]
fn the_archive_holds_only_the_binary_and_the_license() {
    let fixture = Fixture::new();
    fixture.run().unwrap();

    let entries = entries_of(&fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz"));

    let names: Vec<&str> = entries.iter().map(|(name, _, _)| name.as_str()).collect();
    assert_eq!(names, ["porthmos", "LICENSE"]);
    assert_eq!(entries[1].2, LICENSE);
}

#[test]
fn the_binary_keeps_its_bytes_and_is_executable() {
    let fixture = Fixture::new();
    #[cfg(unix)]
    std::fs::set_permissions(fixture.binary(), std::fs::Permissions::from_mode(0o600)).unwrap();
    fixture.run().unwrap();

    let entries = entries_of(&fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz"));

    assert_eq!(entries[0].2, BINARY);
    assert_eq!(entries[0].1 & 0o777, 0o755);
}

#[test]
fn the_sidecar_checksum_matches_the_archive_and_has_the_exact_line_format() {
    let fixture = Fixture::new();
    fixture.run().unwrap();

    let sidecar =
        std::fs::read_to_string(fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz.sha256")).unwrap();

    let archive = std::fs::read(fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz")).unwrap();
    assert_eq!(sidecar, format!("{}  porthmos-x86_64-unknown-linux-gnu.tar.gz\n", sha256_hex(&archive)));
}

#[test]
fn packaging_the_same_inputs_twice_gives_the_same_bytes() {
    let fixture = Fixture::new();
    fixture.run().unwrap();
    let first = std::fs::read(fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz")).unwrap();

    fixture.run().unwrap();

    assert_eq!(std::fs::read(fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz")).unwrap(), first);
}

#[test]
fn a_missing_binary_is_an_error_naming_the_path_and_leaves_no_archive_behind() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.binary()).unwrap();

    let message = fixture.run().unwrap_err().to_string();

    assert!(message.contains(fixture.binary().to_str().unwrap()), "{message}");
    assert_eq!(std::fs::read_dir(fixture.out()).unwrap().count(), 0);
}

const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";

fn windows_run(fixture: &Fixture, out: &Path) -> Result<()> {
    package(WINDOWS_TARGET, &Version::new(0, 4, 0), &fixture.binary(), &fixture.license(), out)
}

fn zip_members(archive: &Path) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(File::open(archive).unwrap()).unwrap();
    (0..archive.len())
        .map(|index| {
            let mut member = archive.by_index(index).unwrap();
            let mut bytes = Vec::new();
            member.read_to_end(&mut bytes).unwrap();
            (member.name().to_string(), bytes)
        })
        .collect()
}

#[test]
fn a_windows_target_is_packaged_as_a_zip_with_the_exe_and_the_license() {
    let fixture = Fixture::new();

    windows_run(&fixture, &fixture.out()).unwrap();

    let mut names: Vec<String> = std::fs::read_dir(fixture.out())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "porthmos-0.4.0-x86_64-pc-windows-msvc.zip",
            "porthmos-x86_64-pc-windows-msvc.zip",
            "porthmos-x86_64-pc-windows-msvc.zip.sha256",
        ]
    );
    let members = zip_members(&fixture.out().join("porthmos-x86_64-pc-windows-msvc.zip"));
    assert_eq!(members, [("porthmos.exe".to_string(), BINARY.to_vec()), ("LICENSE".to_string(), LICENSE.to_vec())]);
}

#[test]
fn a_windows_zip_is_byte_for_byte_reproducible() {
    let fixture = Fixture::new();
    let (first, second) = (fixture.out().join("first"), fixture.out().join("second"));

    windows_run(&fixture, &first).unwrap();
    windows_run(&fixture, &second).unwrap();

    assert_eq!(
        std::fs::read(first.join("porthmos-x86_64-pc-windows-msvc.zip")).unwrap(),
        std::fs::read(second.join("porthmos-x86_64-pc-windows-msvc.zip")).unwrap()
    );
}

#[test]
fn the_windows_checksum_file_names_the_plain_zip() {
    let fixture = Fixture::new();
    windows_run(&fixture, &fixture.out()).unwrap();

    let sidecar = std::fs::read_to_string(fixture.out().join("porthmos-x86_64-pc-windows-msvc.zip.sha256")).unwrap();

    let archive = std::fs::read(fixture.out().join("porthmos-x86_64-pc-windows-msvc.zip")).unwrap();
    assert_eq!(sidecar, format!("{}  porthmos-x86_64-pc-windows-msvc.zip\n", sha256_hex(&archive)));
}

#[test]
fn the_versioned_and_version_less_windows_zips_are_identical() {
    let fixture = Fixture::new();
    windows_run(&fixture, &fixture.out()).unwrap();

    assert_eq!(
        std::fs::read(fixture.out().join("porthmos-0.4.0-x86_64-pc-windows-msvc.zip")).unwrap(),
        std::fs::read(fixture.out().join("porthmos-x86_64-pc-windows-msvc.zip")).unwrap()
    );
}

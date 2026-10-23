use std::{
    fs::File,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

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

    let sidecar = std::fs::read_to_string(fixture.out().join("porthmos-x86_64-unknown-linux-gnu.tar.gz.sha256")).unwrap();

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

use chrono::TimeZone;
use porthmos_update::sha256_hex;

use super::*;

const BASE: &str = "https://github.com/Smyrnis/porthmos/releases/download/v0.4.0";

fn released() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap()
}

fn artifacts(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    for (name, bytes) in files {
        std::fs::write(directory.path().join(name), bytes).unwrap();
    }
    directory
}

fn build(version: &Version, base: &str, directory: &tempfile::TempDir) -> Result<Manifest> {
    build_manifest(version, released(), "notes", base, directory.path())
}

#[test]
fn each_archive_becomes_a_target_with_its_url_hash_and_size() {
    let linux: &[u8] = b"linux archive";
    let mac: &[u8] = b"mac archive bytes";
    let directory = artifacts(&[
        ("porthmos-0.4.0-x86_64-unknown-linux-gnu.tar.gz", linux),
        ("porthmos-0.4.0-aarch64-apple-darwin.tar.gz", mac),
    ]);

    let manifest = build(&Version::new(0, 4, 0), BASE, &directory).unwrap();

    assert_eq!(manifest.version, Version::new(0, 4, 0));
    assert_eq!(manifest.released, released());
    assert_eq!(manifest.notes, "notes");
    let linux_target = &manifest.targets["x86_64-unknown-linux-gnu"];
    assert_eq!(linux_target.url, format!("{BASE}/porthmos-0.4.0-x86_64-unknown-linux-gnu.tar.gz"));
    assert_eq!(linux_target.sha256, sha256_hex(linux));
    assert_eq!(linux_target.size, linux.len() as u64);
    assert_eq!(manifest.targets["aarch64-apple-darwin"].size, mac.len() as u64);
    assert_eq!(manifest.targets.len(), 2);
}

#[test]
fn a_version_with_a_prerelease_part_still_finds_the_target() {
    let version: Version = "0.4.0-rc.1".parse().unwrap();
    let directory = artifacts(&[("porthmos-0.4.0-rc.1-aarch64-apple-darwin.tar.gz", b"x")]);

    let manifest = build(&version, BASE, &directory).unwrap();

    assert_eq!(manifest.targets.keys().collect::<Vec<_>>(), ["aarch64-apple-darwin"]);
}

#[test]
fn version_less_copies_and_checksum_files_are_ignored() {
    let directory = artifacts(&[
        ("porthmos-0.4.0-x86_64-unknown-linux-gnu.tar.gz", b"a"),
        ("porthmos-x86_64-unknown-linux-gnu.tar.gz", b"a"),
        ("porthmos-x86_64-unknown-linux-gnu.tar.gz.sha256", b"line"),
        ("notes.txt", b"n"),
    ]);

    let manifest = build(&Version::new(0, 4, 0), BASE, &directory).unwrap();

    assert_eq!(manifest.targets.len(), 1);
}

#[test]
fn a_zip_archive_is_recognised_too() {
    let directory = artifacts(&[("porthmos-0.4.0-x86_64-pc-windows-msvc.zip", b"z")]);

    let manifest = build(&Version::new(0, 4, 0), BASE, &directory).unwrap();

    assert!(manifest.targets.contains_key("x86_64-pc-windows-msvc"));
}

#[test]
fn two_archives_for_the_same_target_are_an_error() {
    let directory = artifacts(&[
        ("porthmos-0.4.0-x86_64-pc-windows-msvc.tar.gz", b"a"),
        ("porthmos-0.4.0-x86_64-pc-windows-msvc.zip", b"b"),
    ]);

    let message = build(&Version::new(0, 4, 0), BASE, &directory).unwrap_err().to_string();

    assert!(message.contains("x86_64-pc-windows-msvc"), "{message}");
}

#[test]
fn a_folder_without_archives_is_an_error() {
    let directory = artifacts(&[("porthmos-x86_64-unknown-linux-gnu.tar.gz", b"a")]);

    assert!(build(&Version::new(0, 4, 0), BASE, &directory).is_err());
}

#[test]
fn the_base_url_may_end_with_a_slash() {
    let directory = artifacts(&[("porthmos-0.4.0-aarch64-apple-darwin.tar.gz", b"x")]);

    let manifest = build(&Version::new(0, 4, 0), &format!("{BASE}/"), &directory).unwrap();

    assert_eq!(
        manifest.targets["aarch64-apple-darwin"].url,
        format!("{BASE}/porthmos-0.4.0-aarch64-apple-darwin.tar.gz")
    );
}

#[test]
fn a_base_url_that_is_not_https_is_an_error() {
    let directory = artifacts(&[("porthmos-0.4.0-aarch64-apple-darwin.tar.gz", b"x")]);

    assert!(build(&Version::new(0, 4, 0), "http://example.com", &directory).is_err());
}

#[test]
fn an_empty_archive_is_an_error() {
    let directory = artifacts(&[("porthmos-0.4.0-aarch64-apple-darwin.tar.gz", b"")]);

    assert!(build(&Version::new(0, 4, 0), BASE, &directory).is_err());
}

#[test]
fn the_result_survives_manifest_parse() {
    let directory = artifacts(&[("porthmos-0.4.0-aarch64-apple-darwin.tar.gz", b"x")]);
    let manifest = build(&Version::new(0, 4, 0), BASE, &directory).unwrap();

    assert_eq!(Manifest::parse(&manifest.to_json().unwrap()).unwrap(), manifest);
}

use std::sync::atomic::AtomicBool;

use porthmos_vfs::{VfsPath, testing::FakeFs};
use sha2::Sha256;

use super::*;

const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

async fn hash(fs: &FakeFs, path: &str) -> Result<Option<Digest>, ProtocolError> {
    sha256(fs, &VfsPath::new(path), &AtomicBool::new(false), &mut |_| {}).await
}

#[tokio::test]
async fn the_empty_file_has_the_known_digest() {
    let fs = FakeFs::new();
    fs.file("/d/empty", b"", Some(1));

    let digest = hash(&fs, "/d/empty").await.unwrap().unwrap();

    assert_eq!(digest, Digest { hex: EMPTY.to_string(), bytes: 0 });
}

#[tokio::test]
async fn abc_has_the_known_digest() {
    let fs = FakeFs::new();
    fs.file("/d/abc", b"abc", Some(1));

    let digest = hash(&fs, "/d/abc").await.unwrap().unwrap();

    assert_eq!(digest, Digest { hex: ABC.to_string(), bytes: 3 });
}

#[tokio::test]
async fn a_file_longer_than_one_chunk_matches_a_reference_digest() {
    let data: Vec<u8> = (0..200_000u32).map(|index| index as u8).collect();
    let fs = FakeFs::new();
    fs.file("/d/big", &data, Some(1));

    let digest = hash(&fs, "/d/big").await.unwrap().unwrap();

    assert_eq!(digest.hex, format!("{:x}", Sha256::digest(&data)));
    assert_eq!(digest.bytes, 200_000);
}

#[tokio::test]
async fn cancelling_before_the_first_chunk_returns_none() {
    let fs = FakeFs::new();
    fs.file("/d/abc", b"abc", Some(1));

    let result = sha256(&fs, &VfsPath::new("/d/abc"), &AtomicBool::new(true), &mut |_| {}).await;

    assert_eq!(result.unwrap(), None);
}

#[tokio::test]
async fn a_read_that_fails_returns_the_error() {
    let fs = FakeFs::new();
    fs.file("/d/bad", b"abc", Some(1));
    fs.fail_reads("/d/bad");

    assert!(hash(&fs, "/d/bad").await.is_err());
}

#[tokio::test]
async fn a_file_that_cannot_be_opened_returns_the_error() {
    let fs = FakeFs::new();
    fs.file("/d/locked", b"abc", Some(1));
    fs.deny_open("/d/locked");

    assert!(hash(&fs, "/d/locked").await.is_err());
}

#[tokio::test]
async fn a_missing_file_returns_the_error() {
    let fs = FakeFs::new();

    assert!(hash(&fs, "/d/gone").await.is_err());
}

#[tokio::test]
async fn progress_ends_with_the_total_and_never_goes_down() {
    let fs = FakeFs::new();
    fs.file("/d/big", &vec![7u8; 300_000], Some(1));
    let mut calls: Vec<u64> = Vec::new();

    sha256(&fs, &VfsPath::new("/d/big"), &AtomicBool::new(false), &mut |done| calls.push(done)).await.unwrap();

    assert_eq!(calls.last(), Some(&300_000));
    assert!(calls.windows(2).all(|pair| pair[0] <= pair[1]), "{calls:?}");
}

#[test]
fn expected_digests_are_accepted_in_every_usual_spelling() {
    let upper = ABC.to_uppercase();
    for spelling in [
        ABC.to_string(),
        upper.clone(),
        format!("  {ABC}\n"),
        format!("sha256:{ABC}"),
        format!("SHA256:{upper}"),
        format!("{ABC}  name.iso"),
        format!("{ABC} *name.iso"),
        format!("SHA256 (name.iso) = {ABC}"),
        format!("sha256 (name.iso) = {upper}\n"),
        format!("\\{ABC}  na\\nme.iso"),
    ] {
        assert!(matches_expected(&spelling, ABC), "{spelling:?}");
    }
}

#[test]
fn expected_digests_that_are_wrong_or_malformed_never_match() {
    let wrong_but_valid = "0".repeat(64);
    let non_hex = format!("{}g", &ABC[..63]);
    for spelling in [
        ABC[..63].to_string(),
        format!("{ABC}0"),
        non_hex,
        String::new(),
        "   \n".to_string(),
        wrong_but_valid,
        "sha256:".to_string(),
        "SHA256 (name.iso) = ".to_string(),
        format!("SHA256 (name.iso) = {}", "0".repeat(64)),
        "\\".to_string(),
    ] {
        assert!(!matches_expected(&spelling, ABC), "{spelling:?}");
    }
}

#[tokio::test]
async fn progress_is_reported_per_chunk_when_the_interval_is_zero_and_once_when_it_is_long() {
    let fs = FakeFs::new();
    fs.file("/d/big", &vec![7u8; 300_000], Some(1));
    let path = VfsPath::new("/d/big");

    let mut eager = 0;
    sha256_with_interval(&fs, &path, &AtomicBool::new(false), &mut |_| eager += 1, std::time::Duration::ZERO)
        .await
        .unwrap();
    let mut lazy = 0;
    sha256_with_interval(&fs, &path, &AtomicBool::new(false), &mut |_| lazy += 1, std::time::Duration::from_secs(3600))
        .await
        .unwrap();

    assert!(eager >= 4, "{eager}");
    assert_eq!(lazy, 1);
}

#[tokio::test]
async fn a_stalled_read_stops_when_cancelled_without_waiting_for_the_server() {
    let fs = FakeFs::new();
    fs.file("/d/stuck", b"abcdef", Some(1));
    fs.hanging_reads("/d/stuck", 0);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sha256(&fs, &VfsPath::new("/d/stuck"), &cancel, &mut |_| {}),
    )
    .await
    .expect("the hash returns after the cancel");

    assert_eq!(result.unwrap(), None);
}

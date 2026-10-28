use std::collections::BTreeMap;

use chrono::TimeZone;

use super::*;
use crate::{
    Manifest, generate_key_pair, keys, parse_public_key, sign,
    testing::{Route, TestServer},
};

const TARGET: &str = "x86_64-unknown-linux-gnu";
const OTHER_TARGET: &str = "aarch64-apple-darwin";
const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn artifact() -> Artifact {
    Artifact {
        url: "https://github.com/Smyrnis/porthmos/releases/download/v0.5.0/porthmos-0.5.0-x86_64-unknown-linux-gnu.tar.gz"
            .to_string(),
        sha256: HASH.to_string(),
        size: 4_821_337,
    }
}

fn manifest(version: &str, target: &str) -> Manifest {
    Manifest {
        version: version.parse().unwrap(),
        released: Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap(),
        notes: "what changed".to_string(),
        targets: BTreeMap::from([(target.to_string(), artifact())]),
    }
}

struct Release {
    manifest: Vec<u8>,
    signature: String,
    public: String,
}

fn release_of(bytes: Vec<u8>) -> Release {
    let pair = generate_key_pair().unwrap();
    let signature = sign(&pair.private, &bytes).unwrap();
    Release { manifest: bytes, signature, public: pair.public }
}

fn release(version: &str, target: &str) -> Release {
    release_of(manifest(version, target).to_json().unwrap())
}

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

async fn serve(release: &Release) -> TestServer {
    TestServer::start(vec![
        ("/latest.json", Route::Body(release.manifest.clone())),
        ("/latest.json.sig", Route::Body(format!("{}\n", release.signature).into_bytes())),
    ])
    .await
}

fn checker_for(server: &TestServer, trusted_public_key: &str) -> Checker {
    Checker::with_policy(
        &server.url("/latest.json"),
        vec![parse_public_key(trusted_public_key).unwrap()],
        TARGET,
        "porthmos/9.9.9",
        false,
    )
    .unwrap()
}

async fn check(release: &Release, current: &str) -> Result<Outcome, CheckError> {
    let server = serve(release).await;
    checker_for(&server, &release.public).check(&version(current)).await
}

#[tokio::test]
async fn a_newer_signed_manifest_is_available_with_its_details() {
    let release = release("0.5.0", TARGET);

    let outcome = check(&release, "0.4.0").await.unwrap();

    assert_eq!(
        outcome,
        Outcome::Available(UpdateInfo {
            version: version("0.5.0"),
            released: Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap(),
            notes: "what changed".to_string(),
            artifact: artifact(),
        })
    );
}

#[tokio::test]
async fn the_same_version_is_up_to_date() {
    assert_eq!(check(&release("0.4.0", TARGET), "0.4.0").await, Ok(Outcome::UpToDate));
}

#[tokio::test]
async fn an_older_replayed_manifest_is_up_to_date() {
    assert_eq!(check(&release("0.3.0", TARGET), "0.4.0").await, Ok(Outcome::UpToDate));
}

#[tokio::test]
async fn a_pre_release_manifest_is_ignored_by_a_stable_version() {
    assert_eq!(check(&release("0.5.0-rc.1", TARGET), "0.4.0").await, Ok(Outcome::UpToDate));
}

#[tokio::test]
async fn a_release_candidate_user_is_offered_a_newer_candidate() {
    assert!(matches!(check(&release("0.5.0-rc.2", TARGET), "0.5.0-rc.1").await, Ok(Outcome::Available(_))));
}

#[tokio::test]
async fn a_manifest_signed_by_an_untrusted_key_is_rejected() {
    let release = release("0.5.0", TARGET);
    let server = serve(&release).await;
    let stranger = generate_key_pair().unwrap();

    let result = checker_for(&server, &stranger.public).check(&version("0.4.0")).await;

    assert_eq!(result, Err(CheckError::Signature(VerifyError::Mismatch)));
}

#[tokio::test]
async fn a_tampered_manifest_is_rejected() {
    let mut release = release("0.5.0", TARGET);
    release.manifest =
        String::from_utf8(release.manifest).unwrap().replace("what changed", "what chang3d").into_bytes();

    assert_eq!(check(&release, "0.4.0").await, Err(CheckError::Signature(VerifyError::Mismatch)));
}

#[tokio::test]
async fn a_signed_but_malformed_manifest_is_a_manifest_error() {
    let release = release_of(b"{ not json".to_vec());

    assert!(matches!(check(&release, "0.4.0").await, Err(CheckError::Manifest(_))));
}

#[tokio::test]
async fn an_unsigned_malformed_manifest_is_a_signature_error_not_a_manifest_error() {
    let mut release = release("0.5.0", TARGET);
    release.manifest = b"{ not json".to_vec();

    assert_eq!(check(&release, "0.4.0").await, Err(CheckError::Signature(VerifyError::Mismatch)));
}

#[tokio::test]
async fn a_newer_version_without_a_build_for_this_platform_says_so() {
    let release = release("0.5.0", OTHER_TARGET);

    assert_eq!(
        check(&release, "0.4.0").await,
        Err(CheckError::NoBuildForTarget { version: version("0.5.0"), target: TARGET.to_string() })
    );
}

#[tokio::test]
async fn a_missing_signature_file_is_a_fetch_error() {
    let release = release("0.5.0", TARGET);
    let server = TestServer::start(vec![("/latest.json", Route::Body(release.manifest.clone()))]).await;

    let result = checker_for(&server, &release.public).check(&version("0.4.0")).await;

    assert!(matches!(&result, Err(CheckError::Fetch(text)) if text.contains("404")), "{result:?}");
}

#[tokio::test]
async fn a_missing_manifest_is_a_fetch_error() {
    let release = release("0.5.0", TARGET);
    let server =
        TestServer::start(vec![("/latest.json.sig", Route::Body(release.signature.clone().into_bytes()))]).await;

    let result = checker_for(&server, &release.public).check(&version("0.4.0")).await;

    assert!(matches!(&result, Err(CheckError::Fetch(text)) if text.contains("404")), "{result:?}");
}

#[tokio::test]
async fn an_oversized_manifest_is_refused() {
    let release = release_of(vec![b' '; 262_145]);

    let result = check(&release, "0.4.0").await;

    assert!(matches!(&result, Err(CheckError::Fetch(text)) if text.contains("larger than")), "{result:?}");
}

#[tokio::test]
async fn a_binary_signature_body_is_a_bad_encoding_error() {
    let release = release("0.5.0", TARGET);
    let server = TestServer::start(vec![
        ("/latest.json", Route::Body(release.manifest.clone())),
        ("/latest.json.sig", Route::Body(vec![0xff, 0xfe, 0xfd])),
    ])
    .await;

    let result = checker_for(&server, &release.public).check(&version("0.4.0")).await;

    assert_eq!(result, Err(CheckError::Signature(VerifyError::BadEncoding)));
}

#[test]
fn the_default_location_is_the_releases_latest_url_and_is_https() {
    assert_eq!(DEFAULT_MANIFEST_URL, "https://github.com/Smyrnis/porthmos/releases/latest/download/latest.json");
}

#[test]
fn the_release_checker_trusts_exactly_the_embedded_keys() {
    let checker = Checker::release("porthmos/9.9.9").unwrap();

    assert_eq!(checker.trusted_count(), keys::trusted_keys().len());
}

#[tokio::test]
async fn the_checker_asks_with_the_given_user_agent() {
    let release = release("0.5.0", TARGET);
    let server = serve(&release).await;

    checker_for(&server, &release.public).check(&version("0.4.0")).await.unwrap();

    assert_eq!(server.user_agents(), ["porthmos/9.9.9", "porthmos/9.9.9"]);
}

#[tokio::test]
async fn the_production_checker_refuses_plain_http_without_connecting() {
    let release = release("0.5.0", TARGET);
    let server = serve(&release).await;
    let checker = Checker::new(
        &server.url("/latest.json"),
        vec![parse_public_key(&release.public).unwrap()],
        TARGET,
        "porthmos/9.9.9",
    )
    .unwrap();

    let result = checker.check(&version("0.4.0")).await;

    assert!(matches!(&result, Err(CheckError::Fetch(text)) if text.contains("https")), "{result:?}");
    assert_eq!(server.request_count(), 0);
}

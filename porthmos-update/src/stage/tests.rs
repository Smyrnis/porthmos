use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
};

#[cfg(unix)]
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use semver::Version;

use super::*;
use crate::{
    Artifact, sha256_hex,
    testing::{Route, TestServer, archive_with, stub_archive},
};

#[cfg(unix)]
use crate::{
    find_staged, sha256_hex_of_file,
    testing::{stub_script, zip_archive_with},
    unpack::PROGRAM_FILE,
};

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

struct Setup {
    root: tempfile::TempDir,
}

impl Setup {
    fn new() -> Self {
        Setup { root: tempfile::tempdir().unwrap() }
    }

    fn dir(&self) -> UpdateDir {
        UpdateDir::new(self.root.path().join("update"))
    }

    fn stager(&self) -> Stager {
        Stager::with_policy(self.dir(), "porthmos/9.9.9", false).unwrap()
    }

    fn entries(&self) -> Vec<String> {
        let Ok(read) = fs::read_dir(self.dir().path()) else { return Vec::new() };
        let mut names: Vec<String> =
            read.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
}

fn update_for(server: &TestServer, path: &str, version_text: &str, archive: &[u8]) -> UpdateInfo {
    UpdateInfo {
        version: version(version_text),
        released: Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap(),
        notes: String::new(),
        artifact: Artifact { url: server.url(path), sha256: sha256_hex(archive), size: archive.len() as u64 },
    }
}

async fn stage(setup: &Setup, update: &UpdateInfo) -> Result<Staged, StageError> {
    setup.stager().stage(update, &AtomicBool::new(false), &mut |_, _| {}).await
}

#[cfg(unix)]
#[tokio::test]
async fn staging_downloads_unpacks_checks_and_records() {
    let setup = Setup::new();
    let archive = stub_archive("0.5.0", &[("LICENSE", b"MIT")]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);
    let mut seen = Vec::new();

    let staged = setup
        .stager()
        .stage(&update, &AtomicBool::new(false), &mut |done, total| seen.push((done, total)))
        .await
        .unwrap();

    assert_eq!(staged.version, version("0.5.0"));
    assert_eq!(staged.binary, setup.dir().path().join("0.5.0").join(PROGRAM_FILE));
    assert_eq!(staged.size, fs::metadata(&staged.binary).unwrap().len());
    assert_eq!(staged.sha256, sha256_hex_of_file(&staged.binary).unwrap());
    assert_eq!(find_staged(&setup.dir(), &version("0.4.0")), Some(staged));
    assert_eq!(setup.entries(), ["0.5.0", "staged.toml"]);
    assert_eq!(seen.last(), Some(&(archive.len() as u64, archive.len() as u64)));
}

#[tokio::test]
async fn a_download_with_the_wrong_hash_stages_nothing_and_leaves_no_files() {
    let setup = Setup::new();
    let archive = stub_archive("0.5.0", &[]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;
    let mut update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);
    update.artifact.sha256 = sha256_hex(b"something else");

    let result = stage(&setup, &update).await;

    assert!(matches!(result, Err(StageError::Download(_))), "{result:?}");
    assert!(setup.entries().is_empty(), "{:?}", setup.entries());
}

#[tokio::test]
async fn a_truncated_download_stages_nothing_and_leaves_no_files() {
    let setup = Setup::new();
    let archive = stub_archive("0.5.0", &[]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Chunked(archive[..archive.len() / 2].to_vec()))]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);

    let result = stage(&setup, &update).await;

    assert!(matches!(result, Err(StageError::Download(_))), "{result:?}");
    assert!(setup.entries().is_empty(), "{:?}", setup.entries());
}

#[tokio::test]
async fn an_archive_whose_binary_reports_another_version_stages_nothing() {
    let setup = Setup::new();
    let archive = stub_archive("0.4.9", &[]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);

    let result = stage(&setup, &update).await;

    assert!(matches!(result, Err(StageError::HealthCheck(_))), "{result:?}");
    assert!(setup.entries().is_empty(), "{:?}", setup.entries());
}

#[tokio::test]
async fn an_archive_without_the_binary_stages_nothing() {
    let setup = Setup::new();
    let archive = archive_with(&[("LICENSE", b"MIT")]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);

    let result = stage(&setup, &update).await;

    assert!(matches!(result, Err(StageError::Unpack(_))), "{result:?}");
    assert!(setup.entries().is_empty(), "{:?}", setup.entries());
}

#[cfg(unix)]
#[tokio::test]
async fn staging_the_same_version_twice_downloads_only_once() {
    let setup = Setup::new();
    let archive = stub_archive("0.5.0", &[]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);

    let first = stage(&setup, &update).await.unwrap();
    let second = stage(&setup, &update).await.unwrap();

    assert_eq!(first, second);
    assert_eq!(server.request_count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn staging_a_newer_version_replaces_the_older_one() {
    let setup = Setup::new();
    let (old, new) = (stub_archive("0.5.0", &[]), stub_archive("0.6.0", &[]));
    let server =
        TestServer::start(vec![("/old.tar.gz", Route::Body(old.clone())), ("/new.tar.gz", Route::Body(new.clone()))])
            .await;

    stage(&setup, &update_for(&server, "/old.tar.gz", "0.5.0", &old)).await.unwrap();
    let staged = stage(&setup, &update_for(&server, "/new.tar.gz", "0.6.0", &new)).await.unwrap();

    assert_eq!(setup.entries(), ["0.6.0", "staged.toml"]);
    assert_eq!(find_staged(&setup.dir(), &version("0.4.0")), Some(staged));
}

#[cfg(unix)]
#[tokio::test]
async fn a_failed_attempt_keeps_an_earlier_valid_staged_update() {
    let setup = Setup::new();
    let (old, new) = (stub_archive("0.5.0", &[]), stub_archive("0.6.0", &[]));
    let server =
        TestServer::start(vec![("/old.tar.gz", Route::Body(old.clone())), ("/new.tar.gz", Route::Body(new.clone()))])
            .await;
    let earlier = stage(&setup, &update_for(&server, "/old.tar.gz", "0.5.0", &old)).await.unwrap();
    let mut broken = update_for(&server, "/new.tar.gz", "0.6.0", &new);
    broken.artifact.sha256 = sha256_hex(b"wrong");

    let result = stage(&setup, &broken).await;

    assert!(result.is_err());
    assert_eq!(find_staged(&setup.dir(), &version("0.4.0")), Some(earlier));
    assert_eq!(setup.entries(), ["0.5.0", "staged.toml"]);
}

#[tokio::test]
async fn cancelling_stages_nothing_and_cleans_up() {
    let setup = Setup::new();
    let server = TestServer::start(vec![("/p.tar.gz", Route::Endless)]).await;
    let archive_like = vec![0u8; 1_000_000];
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive_like);
    let cancel = AtomicBool::new(false);

    let result = setup
        .stager()
        .stage(&update, &cancel, &mut |done, _| {
            if done >= 2048 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .await;

    assert_eq!(result, Err(StageError::Cancelled));
    assert!(setup.entries().is_empty(), "{:?}", setup.entries());
}

#[tokio::test]
async fn the_production_stager_refuses_plain_http_without_connecting() {
    let setup = Setup::new();
    let archive = stub_archive("0.5.0", &[]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);
    let stager = Stager::new(setup.dir(), "porthmos/9.9.9").unwrap();

    let result = stager.stage(&update, &AtomicBool::new(false), &mut |_, _| {}).await;

    assert!(matches!(&result, Err(StageError::Download(text)) if text.contains("https")), "{result:?}");
    assert_eq!(server.request_count(), 0);
    assert!(setup.entries().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn a_leftover_staging_folder_from_a_crash_does_not_block_staging() {
    let setup = Setup::new();
    let leftover: PathBuf = setup.dir().path().join(format!("0.5.0.staging-{}", std::process::id()));
    fs::create_dir_all(&leftover).unwrap();
    fs::write(leftover.join(PROGRAM_FILE), b"junk").unwrap();
    let archive = stub_archive("0.5.0", &[]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &archive);

    stage(&setup, &update).await.unwrap();

    assert_eq!(setup.entries(), ["0.5.0", "staged.toml"]);
}

#[cfg(unix)]
#[tokio::test]
async fn staging_leaves_the_fresh_work_of_another_process_alone() {
    let setup = Setup::new();
    let other_staging = setup.dir().path().join("0.9.0.staging-4242");
    let other_download = setup.dir().path().join("download.4242");
    for folder in [&other_staging, &other_download] {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join("partial"), b"in progress").unwrap();
    }
    let archive = stub_archive("0.5.0", &[]);
    let server = TestServer::start(vec![("/p.tar.gz", Route::Body(archive.clone()))]).await;

    stage(&setup, &update_for(&server, "/p.tar.gz", "0.5.0", &archive)).await.unwrap();

    assert!(other_staging.join("partial").exists());
    assert!(other_download.join("partial").exists());
}

#[tokio::test]
async fn the_scratch_folder_is_named_after_this_process() {
    let setup = Setup::new();
    let server = TestServer::start(vec![("/p.tar.gz", Route::Endless)]).await;
    let update = update_for(&server, "/p.tar.gz", "0.5.0", &vec![0u8; 1_000_000]);
    let cancel = AtomicBool::new(false);
    let mut scratch_names = Vec::new();
    let root = setup.dir().path().to_path_buf();

    let _ = setup
        .stager()
        .stage(&update, &cancel, &mut |done, _| {
            if scratch_names.is_empty() {
                scratch_names = fs::read_dir(&root)
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect();
            }
            if done >= 2048 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .await;

    assert!(scratch_names.contains(&format!("download.{}", std::process::id())), "{scratch_names:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_staged_update_from_a_zip_is_stored_under_the_platform_program_name() {
    let setup = Setup::new();
    let archive = zip_archive_with(&[(PROGRAM_FILE, &stub_script("0.5.0")), ("LICENSE", b"MIT")]);
    let server = TestServer::start(vec![("/p.zip", Route::Body(archive.clone()))]).await;
    let update = update_for(&server, "/p.zip", "0.5.0", &archive);

    let staged = stage(&setup, &update).await.unwrap();

    assert_eq!(staged.binary, setup.dir().path().join("0.5.0").join(PROGRAM_FILE));
    assert_eq!(find_staged(&setup.dir(), &version("0.4.0")), Some(staged));
}

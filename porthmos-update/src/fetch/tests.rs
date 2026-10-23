use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use super::*;
use crate::testing::{Route, TestServer};

const AGENT: &str = "porthmos/9.9.9";

fn fetcher(timeout: Duration) -> Fetcher {
    Fetcher::new(AGENT, timeout, false).unwrap()
}

fn patient() -> Fetcher {
    fetcher(Duration::from_secs(2))
}

#[tokio::test]
async fn a_body_is_returned_whole() {
    let server = TestServer::start(vec![("/file", Route::Body(b"hello".to_vec()))]).await;

    assert_eq!(patient().get(&server.url("/file"), 1000).await.unwrap(), b"hello");
}

#[tokio::test]
async fn the_user_agent_is_sent_and_nothing_else_identifying_is() {
    let server = TestServer::start(vec![("/file", Route::Body(b"x".to_vec()))]).await;

    patient().get(&server.url("/file"), 1000).await.unwrap();

    assert_eq!(server.user_agents(), [AGENT]);
    let names = server.header_names();
    for forbidden in ["cookie", "authorization", "referer", "x-forwarded-for"] {
        assert!(!names.iter().any(|name| name == forbidden), "{names:?}");
    }
}

#[tokio::test]
async fn a_non_success_status_is_reported_with_its_code() {
    let server = TestServer::start(vec![("/gone", Route::Status(404))]).await;

    assert_eq!(patient().get(&server.url("/gone"), 1000).await, Err(FetchError::Status(404)));
}

#[tokio::test]
async fn a_declared_length_over_the_limit_is_refused_before_reading() {
    let server = TestServer::start(vec![("/big", Route::LargeContentLength(10_000_000_000))]).await;
    let started = Instant::now();

    let result = patient().get(&server.url("/big"), 1000).await;

    assert_eq!(result, Err(FetchError::TooLarge { limit: 1000 }));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn an_endless_body_stops_at_the_limit() {
    let server = TestServer::start(vec![("/endless", Route::Endless)]).await;
    let started = Instant::now();

    let result = patient().get(&server.url("/endless"), 1000).await;

    assert_eq!(result, Err(FetchError::TooLarge { limit: 1000 }));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn a_redirect_is_followed() {
    let server =
        TestServer::start(vec![("/a", Route::Redirect("/b".to_string())), ("/b", Route::Body(b"arrived".to_vec()))])
            .await;

    assert_eq!(patient().get(&server.url("/a"), 1000).await.unwrap(), b"arrived");
}

#[tokio::test]
async fn a_redirected_request_carries_no_referer() {
    let server =
        TestServer::start(vec![("/a", Route::Redirect("/b".to_string())), ("/b", Route::Body(b"arrived".to_vec()))])
            .await;

    patient().get(&server.url("/a?token=secret"), 1000).await.unwrap();

    assert_eq!(server.request_count(), 2);
    assert!(!server.header_names().iter().any(|name| name == "referer"), "{:?}", server.header_names());
}

#[tokio::test]
async fn a_redirect_loop_ends_in_an_error() {
    let server = TestServer::start(vec![("/a", Route::Redirect("/a".to_string()))]).await;
    let started = Instant::now();

    let result = patient().get(&server.url("/a"), 1000).await;

    assert!(matches!(result, Err(FetchError::Network(_))), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn a_server_that_hangs_times_out() {
    let server = TestServer::start(vec![("/hang", Route::Hang)]).await;
    let started = Instant::now();

    let result = fetcher(Duration::from_millis(300)).get(&server.url("/hang"), 1000).await;

    assert!(matches!(&result, Err(FetchError::Network(text)) if text.contains("timed out")), "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn an_http_url_is_refused_without_connecting_when_https_only() {
    let server = TestServer::start(vec![("/file", Route::Body(b"x".to_vec()))]).await;
    let strict = Fetcher::new(AGENT, Duration::from_secs(2), true).unwrap();

    let result = strict.get(&server.url("/file"), 1000).await;

    assert_eq!(result, Err(FetchError::NotHttps));
    assert_eq!(server.request_count(), 0);
}

#[test]
fn redirect_allowed_refuses_plain_http_when_https_only_and_stops_after_five_hops() {
    let secure: reqwest::Url = "https://example.com/x".parse().unwrap();
    let plain: reqwest::Url = "http://example.com/x".parse().unwrap();

    assert!(redirect_allowed(true, &secure, 0));
    assert!(redirect_allowed(true, &secure, 4));
    assert!(!redirect_allowed(true, &secure, 5));
    assert!(!redirect_allowed(true, &plain, 0));
    assert!(redirect_allowed(false, &plain, 0));
}

#[tokio::test]
async fn the_error_text_never_contains_the_url_query() {
    let server = TestServer::start(vec![("/x", Route::Status(500))]).await;

    let error = patient().get(&server.url("/x?token=secret"), 1000).await.unwrap_err();

    assert!(!error.to_string().contains("secret"), "{error}");
}

struct Download {
    folder: tempfile::TempDir,
}

impl Download {
    fn new() -> Self {
        Download { folder: tempfile::tempdir().unwrap() }
    }

    fn path(&self) -> PathBuf {
        self.folder.path().join("archive.part")
    }

    fn leftovers(&self) -> usize {
        std::fs::read_dir(self.folder.path()).unwrap().count()
    }
}

async fn download(
    fetcher: &Fetcher, url: &str, target: &Path, size: u64, sha256: &str,
) -> (Result<(), FetchError>, Vec<(u64, u64)>) {
    let cancel = AtomicBool::new(false);
    let mut seen = Vec::new();
    let result =
        fetcher.get_to_file(url, target, size, sha256, &cancel, &mut |done, total| seen.push((done, total))).await;
    (result, seen)
}

fn body_of(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}

#[tokio::test]
async fn a_matching_download_is_written_and_reported() {
    let body = body_of(50_000);
    let server = TestServer::start(vec![("/archive", Route::Chunked(body.clone()))]).await;
    let target = Download::new();

    let (result, seen) =
        download(&patient(), &server.url("/archive"), &target.path(), 50_000, &crate::sha256_hex(&body)).await;

    assert_eq!(result, Ok(()));
    assert_eq!(std::fs::read(target.path()).unwrap(), body);
    assert_eq!(seen.last(), Some(&(50_000, 50_000)));
    assert!(seen.windows(2).all(|pair| pair[0].0 <= pair[1].0), "{seen:?}");
}

#[tokio::test]
async fn a_wrong_hash_removes_the_file() {
    let body = body_of(1000);
    let server = TestServer::start(vec![("/archive", Route::Body(body.clone()))]).await;
    let target = Download::new();

    let (result, _) =
        download(&patient(), &server.url("/archive"), &target.path(), 1000, &crate::sha256_hex(b"other")).await;

    assert_eq!(result, Err(FetchError::HashMismatch));
    assert_eq!(target.leftovers(), 0);
}

#[tokio::test]
async fn a_body_shorter_than_the_signed_size_is_refused_and_removed() {
    let body = body_of(500);
    let server =
        TestServer::start(vec![("/archive", Route::Chunked(body.clone())), ("/sized", Route::Body(body.clone()))])
            .await;

    for path in ["/archive", "/sized"] {
        let target = Download::new();

        let (result, _) =
            download(&patient(), &server.url(path), &target.path(), 1000, &crate::sha256_hex(&body)).await;

        assert!(matches!(result, Err(FetchError::SizeMismatch { expected: 1000, .. })), "{path}: {result:?}");
        assert_eq!(target.leftovers(), 0, "{path}");
    }
}

#[tokio::test]
async fn a_body_longer_than_the_signed_size_stops_reading_and_removes_the_file() {
    let server = TestServer::start(vec![("/archive", Route::Endless)]).await;
    let target = Download::new();
    let started = Instant::now();

    let (result, _) =
        download(&patient(), &server.url("/archive"), &target.path(), 5000, &crate::sha256_hex(b"x")).await;

    assert!(matches!(result, Err(FetchError::SizeMismatch { expected: 5000, .. })), "{result:?}");
    assert_eq!(target.leftovers(), 0);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn a_declared_length_that_differs_from_the_signed_size_is_refused_before_reading() {
    let server = TestServer::start(vec![("/archive", Route::LargeContentLength(10_000_000_000))]).await;
    let target = Download::new();

    let (result, seen) =
        download(&patient(), &server.url("/archive"), &target.path(), 1000, &crate::sha256_hex(b"x")).await;

    assert!(matches!(result, Err(FetchError::SizeMismatch { expected: 1000, actual: 10_000_000_000 })), "{result:?}");
    assert!(seen.is_empty());
    assert_eq!(target.leftovers(), 0);
}

#[tokio::test]
async fn cancelling_stops_the_download_and_removes_the_file() {
    let server = TestServer::start(vec![("/archive", Route::Endless)]).await;
    let target = Download::new();
    let cancel = AtomicBool::new(false);

    let result = patient()
        .get_to_file(&server.url("/archive"), &target.path(), 1_000_000, "00", &cancel, &mut |done, _| {
            if done >= 2048 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .await;

    assert_eq!(result, Err(FetchError::Cancelled));
    assert_eq!(target.leftovers(), 0);
}

#[tokio::test]
async fn an_http_url_is_refused_when_https_only_and_creates_no_file() {
    let server = TestServer::start(vec![("/archive", Route::Body(b"x".to_vec()))]).await;
    let strict = Fetcher::new(AGENT, Duration::from_secs(2), true).unwrap();
    let target = Download::new();

    let (result, _) = download(&strict, &server.url("/archive"), &target.path(), 1, &crate::sha256_hex(b"x")).await;

    assert_eq!(result, Err(FetchError::NotHttps));
    assert_eq!(server.request_count(), 0);
    assert_eq!(target.leftovers(), 0);
}

#[tokio::test]
async fn a_non_success_status_creates_no_file() {
    let server = TestServer::start(vec![("/archive", Route::Status(404))]).await;
    let target = Download::new();

    let (result, _) = download(&patient(), &server.url("/archive"), &target.path(), 1, &crate::sha256_hex(b"x")).await;

    assert_eq!(result, Err(FetchError::Status(404)));
    assert_eq!(target.leftovers(), 0);
}

#[tokio::test]
async fn an_unwritable_destination_is_a_disk_error() {
    let server = TestServer::start(vec![("/archive", Route::Body(b"x".to_vec()))]).await;
    let target = Download::new();
    let missing_folder = target.folder.path().join("nowhere").join("archive.part");

    let (result, _) = download(&patient(), &server.url("/archive"), &missing_folder, 1, &crate::sha256_hex(b"x")).await;

    assert!(matches!(result, Err(FetchError::Disk(_))), "{result:?}");
}

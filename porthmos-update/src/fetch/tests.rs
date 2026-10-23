use std::time::{Duration, Instant};

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

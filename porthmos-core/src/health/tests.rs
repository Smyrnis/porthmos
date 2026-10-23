use std::time::Duration;

use super::*;

#[tokio::test(start_paused = true)]
async fn waiting_runs_the_full_time_when_nobody_cancels() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let started = tokio::time::Instant::now();

    let finished = wait_unless_cancelled(Duration::from_secs(15), &cancel).await;

    assert!(finished);
    assert!(started.elapsed() >= Duration::from_secs(15));
}

#[tokio::test(start_paused = true)]
async fn waiting_ends_early_when_cancelled() {
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let started = tokio::time::Instant::now();

    let finished = wait_unless_cancelled(Duration::from_secs(600), &cancel).await;

    assert!(!finished);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn waiting_returns_at_once_when_already_cancelled() {
    let cancel = std::sync::atomic::AtomicBool::new(true);

    assert!(!wait_unless_cancelled(Duration::from_secs(600), &cancel).await);
}

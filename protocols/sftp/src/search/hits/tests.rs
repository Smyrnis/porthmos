use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use futures_util::{StreamExt, stream};
use porthmos_vfs::VfsPath;
use tokio::sync::mpsc;

use super::*;

fn entry(path: &str) -> Entry {
    Entry { name: path.rsplit('/').next().unwrap().to_string(), path: VfsPath::new(path), ..Default::default() }
}

fn paths(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("/tree/file{index}.txt")).collect()
}

async fn immediate(path: String) -> Option<Entry> {
    Some(entry(&path))
}

fn drain(mut rx: mpsc::UnboundedReceiver<SearchEvent>) -> Vec<Entry> {
    let mut found = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let SearchEvent::Found(entry) = event {
            found.push(entry);
        }
    }
    found
}

#[tokio::test]
async fn every_path_with_metadata_is_reported() {
    let (tx, rx) = mpsc::unbounded_channel();
    let cancel = AtomicBool::new(false);

    let outcome = stream_hits(stream::iter(paths(5)), immediate, 4, 100, &tx, &cancel).await;

    assert_eq!(outcome, Outcome::Finished { found: 5, truncated: false });
    let mut names: Vec<String> = drain(rx).into_iter().map(|entry| entry.name).collect();
    names.sort();
    assert_eq!(names, ["file0.txt", "file1.txt", "file2.txt", "file3.txt", "file4.txt"]);
}

#[tokio::test]
async fn a_path_whose_metadata_cannot_be_read_is_skipped() {
    let (tx, rx) = mpsc::unbounded_channel();
    let cancel = AtomicBool::new(false);
    let lookup = |path: String| async move { (!path.ends_with("file1.txt")).then(|| entry(&path)) };

    let outcome = stream_hits(stream::iter(paths(3)), lookup, 2, 100, &tx, &cancel).await;

    assert_eq!(outcome, Outcome::Finished { found: 2, truncated: false });
    assert_eq!(drain(rx).len(), 2);
}

#[tokio::test]
async fn metadata_is_fetched_in_parallel() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let cancel = AtomicBool::new(false);
    let lookup = |path: String| async move {
        tokio::time::sleep(Duration::from_millis(40)).await;
        Some(entry(&path))
    };

    let started = Instant::now();
    let outcome = stream_hits(stream::iter(paths(12)), lookup, 6, 100, &tx, &cancel).await;

    assert_eq!(outcome, Outcome::Finished { found: 12, truncated: false });
    assert!(started.elapsed() < Duration::from_millis(300), "took {:?}", started.elapsed());
}

#[tokio::test]
async fn reaching_the_cap_stops_and_reports_truncation() {
    let (tx, rx) = mpsc::unbounded_channel();
    let cancel = AtomicBool::new(false);

    let outcome = stream_hits(stream::iter(paths(50)), immediate, 4, 5, &tx, &cancel).await;

    assert_eq!(outcome, Outcome::Finished { found: 5, truncated: true });
    assert_eq!(drain(rx).len(), 5);
}

#[tokio::test]
async fn paths_beyond_the_cap_are_not_even_read() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let cancel = AtomicBool::new(false);
    let pulled = AtomicUsize::new(0);
    let endless = stream::iter(0..).map(|index| {
        pulled.fetch_add(1, Ordering::SeqCst);
        format!("/tree/file{index}.txt")
    });

    let outcome = stream_hits(Box::pin(endless), immediate, 4, 10, &tx, &cancel).await;

    assert_eq!(outcome, Outcome::Finished { found: 10, truncated: true });
    assert!(pulled.load(Ordering::SeqCst) <= 10 + 4 + 1, "pulled {}", pulled.load(Ordering::SeqCst));
}

#[tokio::test]
async fn results_arrive_before_the_listing_has_ended() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (path_tx, path_rx) = mpsc::unbounded_channel::<String>();
    let cancel = AtomicBool::new(false);
    let incoming =
        stream::unfold(path_rx, |mut path_rx| async move { path_rx.recv().await.map(|path| (path, path_rx)) });

    let search = stream_hits(Box::pin(incoming), immediate, 4, 100, &tx, &cancel);
    let driver = async {
        path_tx.send("/tree/first.txt".to_string()).unwrap();
        let first = rx.recv().await.expect("the first result arrives while the listing is still open");
        drop(path_tx);
        first
    };
    let (outcome, first) = tokio::join!(search, driver);

    assert!(matches!(first, SearchEvent::Found(entry) if entry.name == "first.txt"));
    assert_eq!(outcome, Outcome::Finished { found: 1, truncated: false });
}

#[tokio::test]
async fn a_cancelled_search_stops_without_finishing() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let cancel = AtomicBool::new(true);

    let outcome = stream_hits(stream::iter(paths(5)), immediate, 4, 100, &tx, &cancel).await;

    assert_eq!(outcome, Outcome::Cancelled);
}

#[tokio::test]
async fn a_search_nobody_listens_to_stops() {
    let (tx, rx) = mpsc::unbounded_channel();
    drop(rx);
    let cancel = AtomicBool::new(false);

    let outcome = stream_hits(stream::iter(paths(5)), immediate, 4, 100, &tx, &cancel).await;

    assert_eq!(outcome, Outcome::ReceiverGone);
}

use std::sync::{Arc, atomic::AtomicBool};

use tokio::sync::mpsc;

use super::*;
use crate::testing::FakeFs;

#[test]
fn glob_match_supports_star_and_question_wildcards() {
    assert!(glob_match("*.log", "error.log"));
    assert!(!glob_match("*.log", "error.txt"));
    assert!(glob_match("file?.txt", "file1.txt"));
    assert!(!glob_match("file?.txt", "file12.txt"));
    assert!(glob_match("*", "anything"));
    assert!(glob_match("exact.txt", "exact.txt"));
    assert!(!glob_match("exact.txt", "other.txt"));
    assert!(glob_match("", ""));
    assert!(!glob_match("", "nonempty"));
}

#[test]
fn glob_match_does_not_blow_up_on_pathological_backtracking_patterns() {
    let pattern = "*a".repeat(30) + "*b";
    let name = "a".repeat(40);

    let start = std::time::Instant::now();
    let matched = glob_match(&pattern, &name);
    let elapsed = start.elapsed();

    assert!(!matched);
    assert!(elapsed < std::time::Duration::from_secs(5), "took {elapsed:?}");
}

#[test]
fn glob_match_is_case_insensitive() {
    assert!(glob_match("*.LOG", "error.log"));
    assert!(glob_match("*.log", "ERROR.LOG"));
    assert!(glob_match("File?.txt", "file1.TXT"));
    assert!(glob_match("EXACT.txt", "exact.TXT"));
}

async fn drain(mut rx: mpsc::UnboundedReceiver<SearchEvent>) -> (Vec<Entry>, bool) {
    let mut found = Vec::new();
    let mut truncated = false;
    while let Some(event) = rx.recv().await {
        match event {
            SearchEvent::Found(entry) => found.push(entry),
            SearchEvent::Done { truncated: t } => {
                truncated = t;
                break;
            }
            SearchEvent::Failed(_) => break,
        }
    }
    (found, truncated)
}

#[tokio::test]
async fn search_finds_matching_files_recursively() {
    let fs = FakeFs::new();
    fs.file("/root/a.log", b"x", None).file("/root/sub/b.log", b"x", None).file("/root/c.txt", b"x", None);

    let (tx, rx) = mpsc::unbounded_channel();
    walk_search(
        &fs,
        SearchQuery { root: VfsPath::new("/root"), pattern: "*.log".to_string(), max_depth: 16, max_results: 1000 },
        tx,
        Arc::new(AtomicBool::new(false)),
    )
    .await;

    let (found, truncated) = drain(rx).await;
    assert!(!truncated);
    let mut names: Vec<String> = found.into_iter().map(|e| e.name).collect();
    names.sort();
    assert_eq!(names, vec!["a.log".to_string(), "b.log".to_string()]);
}

#[tokio::test]
async fn search_respects_the_depth_limit() {
    let fs = FakeFs::new();
    fs.file("/root/a/b/c/target.log", b"x", None);

    let (tx, rx) = mpsc::unbounded_channel();
    walk_search(
        &fs,
        SearchQuery { root: VfsPath::new("/root"), pattern: "*.log".to_string(), max_depth: 1, max_results: 1000 },
        tx,
        Arc::new(AtomicBool::new(false)),
    )
    .await;

    let (found, _) = drain(rx).await;
    assert!(found.is_empty());
}

#[tokio::test]
async fn search_reports_truncation_at_the_cap() {
    let fs = FakeFs::new();
    for i in 0..5 {
        fs.file(format!("/root/{i}.log"), b"x", None);
    }

    let (tx, rx) = mpsc::unbounded_channel();
    walk_search(
        &fs,
        SearchQuery { root: VfsPath::new("/root"), pattern: "*.log".to_string(), max_depth: 16, max_results: 3 },
        tx,
        Arc::new(AtomicBool::new(false)),
    )
    .await;

    let (found, truncated) = drain(rx).await;
    assert_eq!(found.len(), 3);
    assert!(truncated);
}

#[tokio::test]
async fn search_stops_promptly_when_already_cancelled() {
    let fs = FakeFs::new();
    fs.file("/root/a.log", b"x", None);
    let cancel = Arc::new(AtomicBool::new(true));

    let (tx, rx) = mpsc::unbounded_channel();
    walk_search(
        &fs,
        SearchQuery { root: VfsPath::new("/root"), pattern: "*.log".to_string(), max_depth: 16, max_results: 1000 },
        tx,
        cancel,
    )
    .await;

    let (found, _) = drain(rx).await;
    assert!(found.is_empty());
}

#[tokio::test]
async fn search_reports_a_root_that_cannot_be_read() {
    let (tx, rx) = mpsc::unbounded_channel();
    walk_search(
        &FakeFs::new(),
        SearchQuery::new(VfsPath::new("/definitely/missing/root"), "*".to_string()),
        tx,
        Arc::new(AtomicBool::new(false)),
    )
    .await;
    let mut rx = rx;
    assert!(matches!(rx.recv().await, Some(SearchEvent::Failed(_))));
}

#[tokio::test]
async fn search_finds_names_that_look_like_windows_paths() {
    let fs = FakeFs::new();
    fs.file("/root/x:1.log", b"x", None).file("/root/a\\b.log", b"x", None).file("/root/con.log", b"x", None);

    let (tx, rx) = mpsc::unbounded_channel();
    walk_search(
        &fs,
        SearchQuery::new(VfsPath::new("/root"), "*.log".to_string()),
        tx,
        Arc::new(AtomicBool::new(false)),
    )
    .await;

    let (found, _) = drain(rx).await;
    let mut paths: Vec<String> = found.iter().map(|entry| entry.path.to_string_lossy().into_owned()).collect();
    paths.sort();
    assert_eq!(paths, vec!["/root/a\\b.log", "/root/con.log", "/root/x:1.log"]);
}

use std::fs;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};

use porthmos_vfs::{Entry, FileSystem, SearchEvent, SearchQuery, VfsPath, walk_search};
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::{LocalFs, from_os_path};

struct Outcome {
    found: Vec<Entry>,
    truncated: Option<bool>,
    failure: Option<String>,
}

fn collect(mut rx: mpsc::UnboundedReceiver<SearchEvent>) -> Outcome {
    let mut outcome = Outcome { found: Vec::new(), truncated: None, failure: None };
    while let Ok(event) = rx.try_recv() {
        match event {
            SearchEvent::Found(entry) => outcome.found.push(entry),
            SearchEvent::Done { truncated } => outcome.truncated = Some(truncated),
            SearchEvent::Failed(message) => outcome.failure = Some(message),
        }
    }
    outcome
}

fn query(root: &Path, pattern: &str) -> SearchQuery {
    SearchQuery::new(from_os_path(root), pattern.to_string())
}

async fn run(query: SearchQuery) -> Outcome {
    let fs = LocalFs::new(std::path::PathBuf::from("/"));
    let (tx, rx) = mpsc::unbounded_channel();
    fs.search(query, tx, Arc::new(AtomicBool::new(false))).await;
    collect(rx)
}

async fn run_generic(query: SearchQuery) -> Outcome {
    let fs = LocalFs::new(std::path::PathBuf::from("/"));
    let (tx, rx) = mpsc::unbounded_channel();
    walk_search(&fs, query, tx, Arc::new(AtomicBool::new(false))).await;
    collect(rx)
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn sample_tree() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("a.txt"), b"a");
    write(&root.join("B.LOG"), b"bb");
    write(&root.join("sub/c.txt"), b"ccc");
    write(&root.join("sub/deep/d.txt"), b"dddd");
    write(&root.join("sub/deep/deeper/e.txt"), b"eeeee");
    write(&root.join("docs.txt/inner.txt"), b"i");
    dir
}

fn paths(outcome: &Outcome) -> Vec<VfsPath> {
    let mut paths: Vec<VfsPath> = outcome.found.iter().map(|entry| entry.path.clone()).collect();
    paths.sort();
    paths
}

fn summary(outcome: &Outcome) -> Vec<(VfsPath, String, bool, u64, Option<u64>)> {
    let mut rows: Vec<_> = outcome
        .found
        .iter()
        .map(|entry| (entry.path.clone(), entry.name.clone(), entry.is_dir, entry.size, entry.modified))
        .collect();
    rows.sort();
    rows
}

#[tokio::test]
async fn search_finds_matches_at_every_depth() {
    let dir = sample_tree();
    let outcome = run(query(dir.path(), "*.txt")).await;

    let expected: Vec<VfsPath> =
        ["a.txt", "sub/c.txt", "sub/deep/d.txt", "sub/deep/deeper/e.txt", "docs.txt", "docs.txt/inner.txt"]
            .iter()
            .map(|relative| from_os_path(&dir.path().join(relative)))
            .collect();
    let mut expected = expected;
    expected.sort();
    assert_eq!(paths(&outcome), expected);
    assert_eq!(outcome.truncated, Some(false));
    assert_eq!(outcome.failure, None);
}

#[tokio::test]
async fn search_matches_names_case_insensitively() {
    let dir = sample_tree();
    let outcome = run(query(dir.path(), "b.log")).await;

    assert_eq!(paths(&outcome), vec![from_os_path(&dir.path().join("B.LOG"))]);
    assert_eq!(outcome.found[0].name, "B.LOG");
}

#[tokio::test]
async fn a_matching_folder_is_reported_as_a_folder_and_is_still_searched() {
    let dir = sample_tree();
    let outcome = run(query(dir.path(), "docs.txt")).await;
    let folder = outcome.found.iter().find(|entry| entry.name == "docs.txt").unwrap();
    assert!(folder.is_dir);

    let inner = run(query(dir.path(), "inner.txt")).await;
    assert_eq!(paths(&inner), vec![from_os_path(&dir.path().join("docs.txt/inner.txt"))]);
}

#[tokio::test]
async fn a_result_carries_its_size_and_modified_time() {
    let dir = sample_tree();
    let outcome = run(query(dir.path(), "d.txt")).await;

    assert_eq!(outcome.found.len(), 1);
    assert_eq!(outcome.found[0].size, 4);
    assert!(!outcome.found[0].is_dir);
    assert!(outcome.found[0].modified.is_some());
}

#[tokio::test]
async fn search_agrees_with_the_generic_walk() {
    let dir = sample_tree();
    for pattern in ["*", "*.txt", "?.txt", "nothing*", "D*", "*e*"] {
        let fast = run(query(dir.path(), pattern)).await;
        let generic = run_generic(query(dir.path(), pattern)).await;
        assert_eq!(summary(&fast), summary(&generic), "pattern {pattern:?}");
        assert_eq!(fast.truncated, generic.truncated, "pattern {pattern:?}");
    }
}

#[tokio::test]
async fn search_honours_the_depth_limit() {
    let dir = sample_tree();
    for max_depth in 0..5 {
        let mut limited = query(dir.path(), "*.txt");
        limited.max_depth = max_depth;
        let fast = run(limited.clone()).await;
        let generic = run_generic(limited).await;
        assert_eq!(summary(&fast), summary(&generic), "depth {max_depth}");
    }
}

#[tokio::test]
async fn search_stops_at_the_result_cap_and_says_so() {
    let dir = sample_tree();
    let mut capped = query(dir.path(), "*.txt");
    capped.max_results = 2;
    let outcome = run(capped.clone()).await;
    let generic = run_generic(capped).await;

    assert_eq!(outcome.found.len(), 2);
    assert_eq!(outcome.truncated, Some(true));
    assert_eq!(outcome.truncated, generic.truncated);
}

#[tokio::test]
async fn search_of_a_missing_folder_fails() {
    let dir = sample_tree();
    let outcome = run(query(&dir.path().join("missing"), "*")).await;

    assert!(outcome.failure.is_some());
    assert_eq!(outcome.truncated, None);
    assert!(outcome.found.is_empty());
}

#[tokio::test]
async fn search_that_is_already_cancelled_reports_nothing() {
    let dir = sample_tree();
    let fs = LocalFs::new(std::path::PathBuf::from("/"));
    let (tx, rx) = mpsc::unbounded_channel();
    fs.search(query(dir.path(), "*"), tx, Arc::new(AtomicBool::new(true))).await;
    let outcome = collect(rx);

    assert!(outcome.found.is_empty());
    assert_eq!(outcome.truncated, None);
}

#[tokio::test]
async fn search_returns_when_nobody_is_listening() {
    let dir = sample_tree();
    let fs = LocalFs::new(std::path::PathBuf::from("/"));
    let (tx, rx) = mpsc::unbounded_channel();
    drop(rx);
    fs.search(query(dir.path(), "*"), tx, Arc::new(AtomicBool::new(false))).await;
}

#[cfg(unix)]
#[tokio::test]
async fn search_does_not_follow_symlinked_folders() {
    let dir = sample_tree();
    std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("sub"), dir.path().join("shortcut")).unwrap();

    let outcome = run(query(dir.path(), "c.txt")).await;
    assert_eq!(paths(&outcome), vec![from_os_path(&dir.path().join("sub/c.txt"))]);

    let links = run(query(dir.path(), "shortcut")).await;
    assert_eq!(links.found.len(), 1);
    assert!(!links.found[0].is_dir);
}

#[cfg(unix)]
#[tokio::test]
async fn search_skips_a_folder_it_cannot_read() {
    use std::os::unix::fs::PermissionsExt;

    let dir = sample_tree();
    let locked = dir.path().join("locked");
    write(&locked.join("hidden.txt"), b"x");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let readable_anyway = fs::read_dir(&locked).is_ok();

    let outcome = run(query(dir.path(), "*.txt")).await;
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(outcome.failure, None);
    assert_eq!(outcome.truncated, Some(false));
    assert!(outcome.found.iter().any(|entry| entry.name == "a.txt"));
    if !readable_anyway {
        assert!(!outcome.found.iter().any(|entry| entry.name == "hidden.txt"));
    }
}

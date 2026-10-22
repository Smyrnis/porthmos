mod support;

use std::{path::Path, sync::Arc, time::Duration};

use porthmos_ftp::Ftp;
use porthmos_vfs::{ErrorKind, FileKind, FileSystem, Protocol};

async fn connect_to(server: &support::Server) -> Arc<dyn FileSystem> {
    let dir = tempfile::tempdir().unwrap();
    Ftp::new(support::store_path(dir.path()))
        .connect(&support::target(server.port, support::USER, Some("pw"), "plain"), &mut support::answers(vec![]))
        .await
        .unwrap()
}

async fn names(fs: &Arc<dyn FileSystem>, dir: &str) -> Vec<String> {
    let mut names: Vec<String> = fs.list(Path::new(dir)).await.unwrap().into_iter().map(|entry| entry.name).collect();
    names.sort();
    names
}

#[tokio::test]
async fn lists_files_and_directories_with_sizes() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("a.txt"), b"12345").unwrap();
    std::fs::create_dir(server.root.path().join("photos")).unwrap();
    let fs = connect_to(&server).await;

    let mut entries = fs.list(Path::new("/")).await.unwrap();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(
        entries.iter().map(|e| (e.name.as_str(), e.is_dir, e.size)).collect::<Vec<_>>(),
        [("a.txt", false, 5), ("photos", true, entries[1].size)]
    );
    assert_eq!(entries[0].path, Path::new("/a.txt"));
}

fn within_a_day_of_now(modified: Option<u64>) -> bool {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    modified.is_some_and(|seconds| seconds.abs_diff(now) < 86_400)
}

#[tokio::test]
async fn list_reports_the_modification_time_of_a_new_file() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("a.txt"), b"12345").unwrap();
    let fs = connect_to(&server).await;

    let entries = fs.list(Path::new("/")).await.unwrap();

    assert!(within_a_day_of_now(entries[0].modified), "{:?}", entries[0].modified);
}

#[tokio::test]
async fn stat_reports_files_directories_and_missing_paths() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("a.txt"), b"12345").unwrap();
    std::fs::create_dir(server.root.path().join("photos")).unwrap();
    let fs = connect_to(&server).await;

    let file = fs.stat(Path::new("/a.txt")).await.unwrap();
    assert_eq!((file.kind, file.size), (FileKind::File, 5));
    assert_eq!(fs.stat(Path::new("/photos")).await.unwrap().kind, FileKind::Dir);
    assert_eq!(fs.stat(Path::new("/missing")).await.unwrap_err().kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn creates_directories() {
    let server = support::start(Some("pw"), None).await;
    let fs = connect_to(&server).await;

    fs.create_dir(Path::new("/new")).await.unwrap();

    assert!(server.root.path().join("new").is_dir());
}

#[tokio::test]
async fn renames_a_file() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("a.txt"), b"a").unwrap();
    let fs = connect_to(&server).await;

    fs.rename(Path::new("/a.txt"), Path::new("/b.txt")).await.unwrap();

    assert_eq!(names(&fs, "/").await, ["b.txt"]);
}

#[tokio::test]
async fn renaming_over_an_existing_file_replaces_it() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("new.txt"), b"new").unwrap();
    std::fs::write(server.root.path().join("old.txt"), b"old").unwrap();
    let fs = connect_to(&server).await;

    fs.rename(Path::new("/new.txt"), Path::new("/old.txt")).await.unwrap();

    assert_eq!(names(&fs, "/").await, ["old.txt"]);
    assert_eq!(std::fs::read(server.root.path().join("old.txt")).unwrap(), b"new");
}

#[tokio::test]
async fn removes_a_file() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("a.txt"), b"a").unwrap();
    let fs = connect_to(&server).await;

    fs.remove_file(Path::new("/a.txt")).await.unwrap();

    assert!(names(&fs, "/").await.is_empty());
}

#[tokio::test]
async fn deletes_a_directory_with_nested_contents() {
    let server = support::start(Some("pw"), None).await;
    let nested = server.root.path().join("tree/inner");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("f.txt"), b"f").unwrap();
    std::fs::write(server.root.path().join("tree/g.txt"), b"g").unwrap();
    let fs = connect_to(&server).await;

    fs.delete(Path::new("/tree")).await.unwrap();

    assert!(!server.root.path().join("tree").exists());
}

#[tokio::test]
async fn a_dropped_idle_connection_is_reopened_transparently() {
    let server = support::start_with(support::Options { password: Some("pw"), tls: None, idle_timeout: Some(1) }).await;
    std::fs::write(server.root.path().join("a.txt"), b"a").unwrap();
    let fs = connect_to(&server).await;

    tokio::time::sleep(Duration::from_millis(2_500)).await;

    assert_eq!(names(&fs, "/").await, ["a.txt"]);
}

#[tokio::test]
async fn remove_dir_removes_an_empty_folder() {
    let server = support::start(Some("pw"), None).await;
    std::fs::create_dir(server.root.path().join("empty")).unwrap();
    let fs = connect_to(&server).await;

    fs.remove_dir(Path::new("/empty")).await.unwrap();

    assert!(!server.root.path().join("empty").exists());
}

#[tokio::test]
async fn remove_dir_refuses_a_folder_with_contents_and_keeps_them() {
    let server = support::start(Some("pw"), None).await;
    std::fs::create_dir_all(server.root.path().join("full/inner")).unwrap();
    std::fs::write(server.root.path().join("full/inner/keep.txt"), b"keep").unwrap();
    let fs = connect_to(&server).await;

    assert!(fs.remove_dir(Path::new("/full")).await.is_err());

    assert!(server.root.path().join("full/inner/keep.txt").exists());
}

#[tokio::test]
async fn remove_dir_refuses_a_file_and_keeps_it() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("a.txt"), b"a").unwrap();
    let fs = connect_to(&server).await;

    assert!(fs.remove_dir(Path::new("/a.txt")).await.is_err());

    assert!(server.root.path().join("a.txt").exists());
}

#[tokio::test]
async fn a_fresh_ftp_session_reports_its_connection_alive() {
    let server = support::start(Some("pw"), None).await;

    let fs = connect_to(&server).await;

    assert_eq!(fs.connection_alive().await, Some(true));
}

#[tokio::test]
async fn a_connection_the_server_dropped_is_found_alive_again_after_reconnecting() {
    let server = support::start_with(support::Options { password: Some("pw"), tls: None, idle_timeout: Some(1) }).await;
    std::fs::write(server.root.path().join("a.txt"), b"x").unwrap();
    let fs = connect_to(&server).await;
    tokio::time::sleep(Duration::from_millis(2_500)).await;

    let alive = fs.connection_alive().await;

    assert_eq!(alive, Some(true));
    assert_eq!(names(&fs, "/").await, vec!["a.txt".to_string()]);
}

#[tokio::test]
async fn listing_gives_the_owner_and_group_the_server_reports() {
    let server = support::start(Some("pw"), None).await;
    std::fs::write(server.root.path().join("a.txt"), b"x").unwrap();
    let fs = connect_to(&server).await;

    let entries = fs.list(Path::new("/")).await.unwrap();

    assert!(entries[0].owner.as_deref().is_some_and(|owner| !owner.is_empty()));
    assert!(entries[0].group.as_deref().is_some_and(|group| !group.is_empty()));
}

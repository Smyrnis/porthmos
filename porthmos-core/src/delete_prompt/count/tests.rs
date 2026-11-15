use std::sync::atomic::{AtomicBool, Ordering};

use porthmos_vfs::{VfsPath, testing::FakeFs};

use super::*;

async fn count(fs: &FakeFs, folders: &[&str], limit: u64) -> ContentCount {
    let paths: Vec<VfsPath> = folders.iter().map(|path| VfsPath::new(path)).collect();
    count_contents(fs, &paths, limit, &AtomicBool::new(false)).await.unwrap().unwrap()
}

fn counted(files: u64, folders: u64) -> ContentCount {
    ContentCount { files, folders, unreadable: 0, capped: false }
}

#[tokio::test]
async fn an_empty_folder_holds_nothing() {
    let fs = FakeFs::new();
    fs.dir("/d/empty");

    assert_eq!(count(&fs, &["/d/empty"], COUNT_LIMIT).await, counted(0, 0));
}

#[tokio::test]
async fn files_and_folders_at_every_depth_are_counted_but_not_the_folder_itself() {
    let fs = FakeFs::new();
    fs.file("/d/top/a.txt", b"a", Some(1));
    fs.file("/d/top/sub/b.txt", b"b", Some(1));
    fs.file("/d/top/sub/deeper/c.txt", b"c", Some(1));
    fs.dir("/d/top/sub/empty");

    assert_eq!(count(&fs, &["/d/top"], COUNT_LIMIT).await, counted(3, 3));
}

#[tokio::test]
async fn several_folders_are_added_together() {
    let fs = FakeFs::new();
    fs.file("/d/one/a", b"a", Some(1));
    fs.file("/d/two/b", b"b", Some(1));
    fs.file("/d/two/inner/c", b"c", Some(1));

    assert_eq!(count(&fs, &["/d/one", "/d/two"], COUNT_LIMIT).await, counted(3, 1));
}

#[tokio::test]
async fn a_symlink_counts_as_one_item_and_is_never_followed() {
    let fs = FakeFs::new();
    fs.file("/d/top/real/inside.txt", b"x", Some(1));
    fs.symlink("/d/top/link", "/d/top/real");

    assert_eq!(count(&fs, &["/d/top"], COUNT_LIMIT).await, counted(2, 1));
}

#[tokio::test]
async fn a_folder_that_cannot_be_read_is_counted_as_unreadable_and_the_rest_still_counts() {
    let fs = FakeFs::new();
    fs.file("/d/top/ok.txt", b"x", Some(1));
    fs.dir("/d/top/locked");
    fs.fail_read_dir("/d/top/locked");

    let result = count(&fs, &["/d/top"], COUNT_LIMIT).await;

    assert_eq!(result, ContentCount { files: 1, folders: 1, unreadable: 1, capped: false });
}

#[tokio::test]
async fn a_top_folder_that_cannot_be_read_is_unreadable_not_an_error() {
    let fs = FakeFs::new();
    fs.dir("/d/locked");
    fs.fail_read_dir("/d/locked");

    let result = count(&fs, &["/d/locked"], COUNT_LIMIT).await;

    assert_eq!(result, ContentCount { files: 0, folders: 0, unreadable: 1, capped: false });
}

#[tokio::test]
async fn a_folder_that_does_not_exist_is_an_error() {
    let fs = FakeFs::new();

    let result = count_contents(&fs, &[VfsPath::new("/d/gone")], COUNT_LIMIT, &AtomicBool::new(false)).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn the_walk_stops_past_the_limit_and_says_so() {
    let fs = FakeFs::new();
    for index in 0..50 {
        fs.file(format!("/d/big/f{index:02}"), b"x", Some(1));
    }

    let result = count(&fs, &["/d/big"], 20).await;

    assert!(result.capped);
    assert!(result.files + result.folders > 20 && result.files + result.folders <= 51, "{result:?}");
}

#[tokio::test]
async fn exactly_the_limit_is_not_capped() {
    let fs = FakeFs::new();
    for index in 0..20 {
        fs.file(format!("/d/exact/f{index:02}"), b"x", Some(1));
    }

    assert_eq!(count(&fs, &["/d/exact"], 20).await, counted(20, 0));
}

#[tokio::test]
async fn a_cancelled_walk_gives_no_answer() {
    let fs = FakeFs::new();
    fs.file("/d/top/a", b"a", Some(1));
    let cancel = AtomicBool::new(true);

    let result = count_contents(&fs, &[VfsPath::new("/d/top")], COUNT_LIMIT, &cancel).await.unwrap();

    assert_eq!(result, None);
    assert!(cancel.load(Ordering::Relaxed));
}

#[test]
fn the_waiting_text_says_it_is_counting() {
    assert_eq!(contents_line(&Contents::Counting), "Counting what is inside\u{2026}");
}

#[test]
fn an_unavailable_count_says_so_without_a_number() {
    assert_eq!(contents_line(&Contents::Unavailable), "Couldn't count what is inside.");
}

#[test]
fn an_empty_result_says_the_folders_are_empty() {
    assert_eq!(contents_line(&Contents::Counted(counted(0, 0))), "Inside: nothing.");
}

#[test]
fn files_and_folders_are_named_with_the_right_plural() {
    let line = |files, folders| contents_line(&Contents::Counted(counted(files, folders)));

    assert_eq!(line(1, 0), "Inside: 1 file.");
    assert_eq!(line(2, 0), "Inside: 2 files.");
    assert_eq!(line(0, 1), "Inside: 1 folder.");
    assert_eq!(line(0, 3), "Inside: 3 folders.");
    assert_eq!(line(1, 1), "Inside: 1 file and 1 folder.");
    assert_eq!(line(1204, 37), "Inside: 1,204 files and 37 folders.");
    assert_eq!(line(1_250_000, 0), "Inside: 1,250,000 files.");
}

#[test]
fn a_capped_count_says_more_than_the_limit() {
    let capped = ContentCount { files: 9_000, folders: 1_001, unreadable: 0, capped: true };

    assert_eq!(contents_line(&Contents::Counted(capped)), "Inside: more than 10,000 items.");
}

#[test]
fn unreadable_folders_make_the_count_a_minimum() {
    let partly = ContentCount { files: 12, folders: 2, unreadable: 1, capped: false };
    let several = ContentCount { files: 0, folders: 0, unreadable: 3, capped: false };

    assert_eq!(
        contents_line(&Contents::Counted(partly)),
        "Inside: at least 12 files and 2 folders; 1 folder could not be read."
    );
    assert_eq!(contents_line(&Contents::Counted(several)), "Inside: nothing found; 3 folders could not be read.");
}

#[test]
fn a_capped_count_with_unreadable_folders_keeps_only_the_cap_wording() {
    let capped = ContentCount { files: 9_000, folders: 1_001, unreadable: 2, capped: true };

    assert_eq!(contents_line(&Contents::Counted(capped)), "Inside: more than 10,000 items.");
}

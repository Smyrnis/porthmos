use std::sync::atomic::AtomicBool;
use std::time::Duration;

use porthmos_vfs::{VfsPath, testing::FakeFs};

use super::*;
use crate::delete_prompt::count::COUNT_LIMIT;

async fn run(fs: &FakeFs, roots: &[&str]) -> (Measured, Vec<Measured>) {
    let paths: Vec<VfsPath> = roots.iter().map(|path| VfsPath::new(path)).collect();
    let mut calls = Vec::new();
    let result = measure(fs, &paths, &AtomicBool::new(false), &mut |measured| calls.push(*measured)).await;
    (result.unwrap().unwrap(), calls)
}

fn measured(bytes: u64, files: u64, folders: u64) -> Measured {
    Measured { bytes, files, folders, ..Measured::default() }
}

#[tokio::test]
async fn a_file_root_adds_its_size() {
    let fs = FakeFs::new();
    fs.file("/a", b"12345", Some(1));

    assert_eq!(run(&fs, &["/a"]).await.0, measured(5, 1, 0));
}

#[tokio::test]
async fn an_empty_folder_root_is_all_zero() {
    let fs = FakeFs::new();
    fs.dir("/d/empty");

    assert_eq!(run(&fs, &["/d/empty"]).await.0, Measured::default());
}

#[tokio::test]
async fn nested_folders_add_up_without_counting_the_root() {
    let fs = FakeFs::new();
    fs.file("/d/a", b"123", Some(1)).file("/d/s/b", b"1234", Some(1)).file("/d/s/t/c", b"12345", Some(1));

    assert_eq!(run(&fs, &["/d"]).await.0, measured(12, 3, 2));
}

#[tokio::test]
async fn several_roots_are_summed() {
    let fs = FakeFs::new();
    fs.file("/one/a", b"12", Some(1)).file("/two", b"123", Some(1)).file("/three/x/y", b"1", Some(1));

    assert_eq!(run(&fs, &["/one", "/two", "/three"]).await.0, measured(6, 3, 1));
}

#[tokio::test]
async fn a_symlink_is_counted_not_followed_and_not_sized() {
    let fs = FakeFs::new();
    fs.file("/d/a", b"123", Some(1));
    fs.symlink("/d/loop", "/d");

    let (total, _) = run(&fs, &["/d"]).await;

    assert_eq!(total, Measured { bytes: 3, files: 1, symlinks: 1, ..Measured::default() });
}

#[tokio::test]
async fn a_locked_folder_counts_as_unreadable_and_the_walk_continues() {
    let fs = FakeFs::new();
    fs.file("/d/a", b"12", Some(1)).dir("/d/locked").file("/d/z/b", b"3", Some(1));
    fs.fail_read_dir("/d/locked");

    let (total, _) = run(&fs, &["/d"]).await;

    assert_eq!(total, Measured { bytes: 3, files: 2, folders: 2, unreadable: 1, symlinks: 0 });
}

#[tokio::test]
async fn a_missing_root_is_an_error() {
    let fs = FakeFs::new();

    let result = measure(&fs, &[VfsPath::new("/d/gone")], &AtomicBool::new(false), &mut |_| {}).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn another_listing_error_mid_walk_aborts_instead_of_returning_a_smaller_total() {
    let fs = FakeFs::new();
    fs.file("/d/a", b"12", Some(1)).file("/d/deep/b", b"3", Some(1));
    fs.fail_read_dir_with("/d/deep", porthmos_vfs::ErrorKind::Other);

    let result = measure(&fs, &[VfsPath::new("/d")], &AtomicBool::new(false), &mut |_| {}).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn a_folder_larger_than_the_delete_cap_is_measured_completely() {
    let fs = FakeFs::new();
    let count = COUNT_LIMIT + 2_000;
    for index in 0..count {
        fs.file(format!("/d/big/f{index:05}"), b"x", Some(1));
    }

    let (total, _) = run(&fs, &["/d/big"]).await;

    assert_eq!(total, measured(count, count, 0));
}

#[tokio::test]
async fn a_cancelled_walk_returns_none() {
    let fs = FakeFs::new();
    fs.file("/d/a", b"1", Some(1));
    let cancel = AtomicBool::new(true);

    let result = measure(&fs, &[VfsPath::new("/d")], &cancel, &mut |_| {}).await.unwrap();

    assert_eq!(result, None);
}

#[tokio::test]
async fn progress_is_throttled_and_ends_with_the_final_value() {
    let fs = FakeFs::new();
    for folder in 0..300 {
        for file in 0..10 {
            fs.file(format!("/d/f{folder:03}/g{file}"), b"x", Some(1));
        }
    }

    let (total, calls) = run(&fs, &["/d"]).await;

    assert!(calls.len() < 50, "{} progress calls", calls.len());
    assert_eq!(calls.last(), Some(&total));
    assert!(calls.windows(2).all(|pair| pair[0].bytes <= pair[1].bytes));
}

#[test]
fn a_size_past_u64_max_saturates() {
    let mut total = Measured { bytes: u64::MAX - 1, ..Measured::default() };

    total.add_file(10);

    assert_eq!(total.bytes, u64::MAX);
    assert_eq!(total.files, 1);
}

#[tokio::test]
async fn a_symlink_root_is_counted_and_never_followed() {
    let fs = FakeFs::new();
    fs.file("/d/target/big", &[0u8; 1_000], Some(1));
    fs.symlink("/d/link", "/d/target");

    let (total, _) = run(&fs, &["/d/link"]).await;

    assert_eq!(total, Measured { symlinks: 1, ..Measured::default() });
}

#[tokio::test]
async fn roots_beside_each_other_with_a_link_among_them_are_each_judged_by_their_own_kind() {
    let fs = FakeFs::new();
    fs.file("/d/real/a", b"123", Some(1)).file("/d/plain", b"45", Some(1)).file(
        "/d/target/big",
        &[0u8; 1_000],
        Some(1),
    );
    fs.symlink("/d/link", "/d/target");

    let (total, _) = run(&fs, &["/d/real", "/d/plain", "/d/link"]).await;

    assert_eq!(total, Measured { bytes: 5, files: 2, symlinks: 1, ..Measured::default() });
}

#[tokio::test]
async fn the_filesystem_root_is_walked_not_looked_up_in_a_parent() {
    let fs = FakeFs::new();
    fs.file("/a", b"12", Some(1));

    let (total, _) = run(&fs, &["/"]).await;

    assert_eq!((total.bytes, total.files), (2, 1));
}

#[tokio::test]
async fn progress_is_reported_after_every_folder_when_the_interval_is_zero_and_once_when_it_is_long() {
    let fs = FakeFs::new();
    for folder in 0..20 {
        fs.file(format!("/d/f{folder:02}/g"), b"x", Some(1));
    }
    let roots = [VfsPath::new("/d")];

    let mut eager = 0;
    measure_with_interval(&fs, &roots, &AtomicBool::new(false), &mut |_| eager += 1, Duration::ZERO).await.unwrap();
    let mut lazy = 0;
    measure_with_interval(&fs, &roots, &AtomicBool::new(false), &mut |_| lazy += 1, Duration::from_secs(3600))
        .await
        .unwrap();

    assert!(eager > 20, "{eager}");
    assert_eq!(lazy, 1);
}

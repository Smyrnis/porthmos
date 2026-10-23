use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use porthmos_vfs::{ErrorKind, testing::FakeFs};

use super::*;

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

fn text(content: &str) -> PreviewContent {
    PreviewContent::Text(content.to_string())
}

#[test]
fn plain_ascii_is_text() {
    assert_eq!(classify(b"hello\nworld\n".to_vec(), false), text("hello\nworld\n"));
}

#[test]
fn multibyte_utf8_is_text() {
    assert_eq!(
        classify("caf\u{e9} \u{4e2d}\u{6587} \u{1f600}".as_bytes().to_vec(), false),
        text("caf\u{e9} \u{4e2d}\u{6587} \u{1f600}")
    );
}

#[test]
fn an_empty_file_is_empty_text() {
    assert_eq!(classify(Vec::new(), false), text(""));
}

#[test]
fn a_nul_byte_makes_it_binary() {
    let bytes = b"text then \0 a nul".to_vec();

    assert_eq!(classify(bytes.clone(), false), PreviewContent::Binary(bytes));
}

#[test]
fn bytes_that_are_not_utf8_are_binary() {
    for bytes in [vec![0xff, 0xfe, 0x41], b"caf\xe9".to_vec(), vec![0xc0, 0x80], vec![0xed, 0xa0, 0x80]] {
        assert_eq!(classify(bytes.clone(), false), PreviewContent::Binary(bytes));
    }
}

#[test]
fn a_cut_in_the_middle_of_a_character_at_the_limit_is_still_text() {
    let mut bytes = "abc\u{4e2d}".as_bytes().to_vec();
    bytes.truncate(bytes.len() - 1);

    assert_eq!(classify(bytes, true), text("abc"));
}

#[test]
fn the_same_cut_without_a_limit_hit_is_binary() {
    let mut bytes = "abc\u{4e2d}".as_bytes().to_vec();
    bytes.truncate(bytes.len() - 1);

    assert_eq!(classify(bytes.clone(), false), PreviewContent::Binary(bytes));
}

#[test]
fn a_bad_byte_before_the_end_is_binary_even_when_truncated() {
    let bytes = vec![b'a', 0xff, b'b', 0xe4, 0xb8];

    assert_eq!(classify(bytes.clone(), true), PreviewContent::Binary(bytes));
}

#[test]
fn a_byte_order_mark_is_not_shown() {
    assert_eq!(classify(b"\xef\xbb\xbfhello".to_vec(), false), text("hello"));
}

#[test]
fn carriage_returns_are_kept_for_the_viewer_to_deal_with() {
    assert_eq!(classify(b"a\r\nb\r\n".to_vec(), false), text("a\r\nb\r\n"));
}

#[test]
fn escape_sequences_are_valid_text_and_left_for_the_viewer_to_neutralise() {
    assert_eq!(classify(b"\x1b[31mred\x1b[0m".to_vec(), false), text("\u{1b}[31mred\u{1b}[0m"));
}

fn fs_with(path: &str, data: &[u8]) -> FakeFs {
    let fs = FakeFs::new();
    fs.file(path, data, Some(5));
    fs
}

#[tokio::test]
async fn a_small_file_is_read_whole() {
    let fs = fs_with("/d/a.txt", b"hello");

    let preview = read_preview(&fs, Path::new("/d/a.txt"), 100, &never()).await.unwrap();

    assert_eq!(preview, Preview { size: Some(5), shown: 5, truncated: false, content: text("hello") });
}

#[tokio::test]
async fn a_file_longer_than_the_limit_is_cut_and_says_so() {
    let fs = fs_with("/d/big.txt", &vec![b'x'; 1_000_000]);

    let preview = read_preview(&fs, Path::new("/d/big.txt"), 1000, &never()).await.unwrap();

    assert_eq!((preview.size, preview.shown, preview.truncated), (Some(1_000_000), 1000, true));
    assert_eq!(preview.content, text(&"x".repeat(1000)));
}

#[tokio::test]
async fn a_file_exactly_the_limit_long_is_not_truncated() {
    let fs = fs_with("/d/a.txt", &vec![b'x'; 1000]);

    let preview = read_preview(&fs, Path::new("/d/a.txt"), 1000, &never()).await.unwrap();

    assert_eq!((preview.shown, preview.truncated), (1000, false));
}

#[tokio::test]
async fn a_file_one_byte_over_the_limit_is_truncated() {
    let fs = fs_with("/d/a.txt", &vec![b'x'; 1001]);

    let preview = read_preview(&fs, Path::new("/d/a.txt"), 1000, &never()).await.unwrap();

    assert_eq!((preview.shown, preview.truncated), (1000, true));
}

#[tokio::test]
async fn a_binary_file_is_kept_as_bytes() {
    let fs = fs_with("/d/a.bin", &[0, 1, 2, 3, 255]);

    let preview = read_preview(&fs, Path::new("/d/a.bin"), 100, &never()).await.unwrap();

    assert_eq!(preview.content, PreviewContent::Binary(vec![0, 1, 2, 3, 255]));
}

#[tokio::test]
async fn a_character_cut_by_the_limit_still_reads_as_text() {
    let fs = fs_with("/d/a.txt", "ab\u{4e2d}\u{6587}".as_bytes());

    let preview = read_preview(&fs, Path::new("/d/a.txt"), 4, &never()).await.unwrap();

    assert_eq!(preview.content, text("ab"));
    assert!(preview.truncated);
}

#[tokio::test]
async fn a_folder_is_refused() {
    let fs = FakeFs::new();
    fs.dir("/d/sub");

    let error = read_preview(&fs, Path::new("/d/sub"), 100, &never()).await.unwrap_err();

    assert!(error.to_string().contains("folder"), "{error}");
}

#[tokio::test]
async fn a_missing_file_is_not_found() {
    let fs = FakeFs::new();

    let error = read_preview(&fs, Path::new("/d/none"), 100, &never()).await.unwrap_err();

    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn a_read_that_fails_is_an_error() {
    let fs = fs_with("/d/a.txt", b"hello");
    fs.fail_reads("/d/a.txt");

    assert!(read_preview(&fs, Path::new("/d/a.txt"), 100, &never()).await.is_err());
}

#[tokio::test]
async fn an_empty_file_is_empty_text_with_size_zero() {
    let fs = fs_with("/d/empty", b"");

    let preview = read_preview(&fs, Path::new("/d/empty"), 100, &never()).await.unwrap();

    assert_eq!(preview, Preview { size: Some(0), shown: 0, truncated: false, content: text("") });
}

#[test]
fn every_cut_inside_a_four_byte_character_is_still_text() {
    let whole = "ab\u{1f600}".as_bytes();
    for keep in 3..whole.len() {
        assert_eq!(classify(whole[..keep].to_vec(), true), text("ab"), "keeping {keep} bytes");
    }
}

#[test]
fn a_truncated_buffer_that_ends_in_a_broken_sequence_is_binary() {
    for tail in [vec![0xed, 0xa0], vec![0xf4, 0x90], vec![0xf5], vec![0xc0], vec![0xc1, 0xbf]] {
        let mut bytes = b"ok ".to_vec();
        bytes.extend(&tail);
        assert_eq!(classify(bytes.clone(), true), PreviewContent::Binary(bytes), "{tail:?}");
    }
}

#[test]
fn a_file_that_is_only_a_byte_order_mark_is_empty_text() {
    assert_eq!(classify(b"\xef\xbb\xbf".to_vec(), false), text(""));
}

#[test]
fn a_byte_order_mark_followed_by_a_cut_character_is_text() {
    let mut bytes = b"\xef\xbb\xbfab".to_vec();
    bytes.extend(&"\u{4e2d}".as_bytes()[..2]);

    assert_eq!(classify(bytes, true), text("ab"));
}

#[tokio::test]
async fn a_nul_just_past_the_limit_does_not_make_a_text_file_binary() {
    let mut data = vec![b'x'; 1000];
    data.push(0);
    let fs = fs_with("/d/a.txt", &data);

    let preview = read_preview(&fs, Path::new("/d/a.txt"), 1000, &never()).await.unwrap();

    assert_eq!(preview.content, text(&"x".repeat(1000)));
    assert!(preview.truncated);
}

#[test]
fn a_size_the_server_reports_too_small_is_treated_as_unknown() {
    assert_eq!(known_size(5000, 3000), Some(5000));
    assert_eq!(known_size(3000, 3000), Some(3000));
    assert_eq!(known_size(0, 3000), None);
    assert_eq!(known_size(100, 3000), None);
    assert_eq!(known_size(0, 0), Some(0));
}

#[test]
fn only_regular_files_and_unknown_modes_are_read() {
    assert!(!is_special(0o100644));
    assert!(!is_special(0o644));
    assert!(!is_special(0));
    assert!(is_special(0o010644));
    assert!(is_special(0o020666));
    assert!(is_special(0o060660));
    assert!(is_special(0o140755));
}

#[tokio::test]
async fn a_named_pipe_is_refused_at_once_instead_of_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let pipe = dir.path().join("pipe");
    let path = std::ffi::CString::new(pipe.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let fs = porthmos_lfs::LocalFs::new(dir.path().to_path_buf());

    let outcome = tokio::time::timeout(Duration::from_secs(2), read_preview(&fs, &pipe, 100, &never())).await;

    let error = outcome.expect("did not block").unwrap_err();
    assert!(error.to_string().contains("not a regular file"), "{error}");
}

#[tokio::test]
async fn a_device_file_is_refused() {
    let fs = porthmos_lfs::LocalFs::new("/".into());

    let error = read_preview(&fs, Path::new("/dev/null"), 100, &never()).await.unwrap_err();

    assert!(error.to_string().contains("not a regular file"), "{error}");
}

#[tokio::test]
async fn a_regular_local_file_still_reads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "local text").unwrap();
    let fs = porthmos_lfs::LocalFs::new(dir.path().to_path_buf());

    let preview = read_preview(&fs, &dir.path().join("a.txt"), 100, &never()).await.unwrap();

    assert_eq!((preview.size, preview.content), (Some(10), text("local text")));
}

#[tokio::test(start_paused = true)]
async fn a_read_that_stalls_gives_up_after_the_stall_time_not_before() {
    let fs = fs_with("/d/a.txt", b"abcdef");
    fs.hanging_reads("/d/a.txt", 3);
    let started = tokio::time::Instant::now();

    let error = read_preview(&fs, Path::new("/d/a.txt"), 100, &never()).await.unwrap_err();

    assert!(error.to_string().contains("took too long"), "{error}");
    assert!(started.elapsed() >= STALL_TIMEOUT && started.elapsed() < STALL_TIMEOUT + Duration::from_secs(1));
}

#[tokio::test(start_paused = true)]
async fn a_slow_but_steady_read_is_never_cut_off() {
    let fs = fs_with("/d/a.txt", &vec![b'x'; 4000]);

    let preview = read_preview(&fs, Path::new("/d/a.txt"), 5000, &never()).await.unwrap();

    assert_eq!(preview.shown, 4000);
}

#[tokio::test(start_paused = true)]
async fn a_stat_that_never_answers_gives_up_after_the_open_time() {
    let fs = fs_with("/d/a.txt", b"x");
    fs.hanging_stat("/d/a.txt");
    let started = tokio::time::Instant::now();

    let error = read_preview(&fs, Path::new("/d/a.txt"), 100, &never()).await.unwrap_err();

    assert!(error.to_string().contains("took too long"), "{error}");
    assert!(started.elapsed() >= OPEN_TIMEOUT);
}

#[tokio::test]
async fn a_read_cancelled_before_it_starts_reads_nothing() {
    let fs = fs_with("/d/a.txt", b"abc");
    let cancel = AtomicBool::new(true);

    let error = read_preview(&fs, Path::new("/d/a.txt"), 100, &cancel).await.unwrap_err();

    assert_eq!(error.kind(), ErrorKind::Cancelled);
}

#[tokio::test(start_paused = true)]
async fn cancelling_a_stalled_read_stops_it_within_a_fraction_of_a_second() {
    let fs = fs_with("/d/a.txt", b"abcdef");
    fs.hanging_reads("/d/a.txt", 3);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let reading = fs.clone();
    let started = tokio::time::Instant::now();
    let task = tokio::spawn(async move { read_preview(&reading, Path::new("/d/a.txt"), 100, &flag).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    cancel.store(true, Ordering::Relaxed);
    let error = task.await.unwrap().unwrap_err();

    assert_eq!(error.kind(), ErrorKind::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
}

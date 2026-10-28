use super::*;

#[test]
fn a_real_folder_has_free_space() {
    let dir = tempfile::tempdir().unwrap();

    let free = free_space(dir.path()).unwrap();

    assert!(free > 0);
}

#[test]
fn free_space_is_the_same_for_a_file_and_its_folder_within_a_small_margin() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("f");
    std::fs::write(&file, b"x").unwrap();

    let in_folder = free_space(dir.path()).unwrap();
    let of_file = free_space(&file).unwrap();

    assert!(in_folder.abs_diff(of_file) < 64 * 1024 * 1024);
}

#[test]
fn a_missing_folder_is_an_error() {
    let dir = tempfile::tempdir().unwrap();

    let error = free_space(&dir.path().join("missing")).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[cfg(windows)]
#[test]
fn an_empty_path_is_invalid_input() {
    let error = free_space(Path::new("")).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn a_path_with_a_nul_byte_is_an_error() {
    let error = free_space(Path::new("a\0b")).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}

#[cfg(unix)]
#[test]
fn available_bytes_multiplies_blocks_by_their_size() {
    assert_eq!(available_bytes(100, 4096), 409_600);
}

#[cfg(unix)]
#[test]
fn available_bytes_cannot_overflow() {
    assert_eq!(available_bytes(u64::MAX, 2), u64::MAX);
}

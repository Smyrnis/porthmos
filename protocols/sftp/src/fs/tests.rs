use porthmos_vfs::{ErrorKind, FileKind};
use russh_sftp::protocol::{FileAttributes, Status, StatusCode};

use super::*;

#[test]
fn the_backoff_covers_every_pipelined_write() {
    use crate::subsystem::{SFTP_MAX_CONCURRENT_WRITES, SFTP_MAX_WRITE_PACKET_LEN};

    assert_eq!(SFTP_RESUME_BACKOFF_BYTES, SFTP_MAX_CONCURRENT_WRITES as u64 * SFTP_MAX_WRITE_PACKET_LEN as u64);
}

#[test]
fn sftp_symlink_metadata_is_reported_as_a_symlink() {
    let mut attributes = FileAttributes::empty();
    attributes.permissions = Some(0o120777);
    attributes.size = Some(7);
    let metadata = metadata_from_sftp(&attributes);
    assert_eq!(metadata.kind, FileKind::Symlink);
    assert_eq!(metadata.size, 7);
}

#[test]
fn sftp_directory_metadata_is_reported_as_a_directory_with_its_mtime() {
    let mut attributes = FileAttributes::empty();
    attributes.permissions = Some(0o040755);
    attributes.mtime = Some(42);
    let metadata = metadata_from_sftp(&attributes);
    assert_eq!((metadata.kind, metadata.modified), (FileKind::Dir, Some(42)));
}

fn status(code: StatusCode) -> russh_sftp::client::error::Error {
    russh_sftp::client::error::Error::Status(Status {
        id: 0,
        status_code: code,
        error_message: "boom".to_string(),
        language_tag: "en".to_string(),
    })
}

#[test]
fn sftp_status_codes_map_to_shared_error_kinds() {
    assert_eq!(sftp_error(status(StatusCode::NoSuchFile)).kind(), ErrorKind::NotFound);
    assert_eq!(sftp_error(status(StatusCode::PermissionDenied)).kind(), ErrorKind::PermissionDenied);
    assert_eq!(sftp_error(status(StatusCode::Failure)).kind(), ErrorKind::Other);
}

fn status_with_message(code: StatusCode, message: &str) -> russh_sftp::client::error::Error {
    russh_sftp::client::error::Error::Status(Status {
        id: 0,
        status_code: code,
        error_message: message.to_string(),
        language_tag: "en".to_string(),
    })
}

#[test]
fn a_server_message_that_repeats_the_status_is_shown_once() {
    let error = sftp_error(status_with_message(StatusCode::PermissionDenied, "Permission denied"));

    assert_eq!(error.to_string(), "Permission denied");
}

#[test]
fn an_empty_server_message_shows_just_the_status() {
    let error = sftp_error(status_with_message(StatusCode::NoSuchFile, ""));

    assert_eq!(error.to_string(), "No such file");
}

#[test]
fn a_server_message_that_adds_something_follows_the_status() {
    let error = sftp_error(status_with_message(StatusCode::Failure, "quota exceeded"));

    assert_eq!(error.to_string(), "Failure: quota exceeded");
}

#[test]
fn the_server_supplied_names_are_used_for_owner_and_group() {
    let mut attributes = FileAttributes::empty();
    attributes.user = Some("nikos".to_string());
    attributes.group = Some("staff".to_string());
    attributes.uid = Some(501);
    attributes.gid = Some(20);

    assert_eq!(ownership_of(&attributes), (Some("nikos".to_string()), Some("staff".to_string())));
}

#[test]
fn numeric_ids_stand_in_when_the_server_gives_no_names() {
    let mut attributes = FileAttributes::empty();
    attributes.uid = Some(501);
    attributes.gid = Some(20);

    assert_eq!(ownership_of(&attributes), (Some("501".to_string()), Some("20".to_string())));
}

#[test]
fn an_empty_name_falls_back_to_the_numeric_id() {
    let mut attributes = FileAttributes::empty();
    attributes.user = Some(String::new());
    attributes.uid = Some(7);

    assert_eq!(ownership_of(&attributes).0, Some("7".to_string()));
}

#[test]
fn owner_and_group_are_unknown_without_names_or_ids() {
    assert_eq!(ownership_of(&FileAttributes::empty()), (None, None));
}

fn statvfs(fragment_size: u64, blocks_avail: u64, blocks_free: u64) -> russh_sftp::extensions::Statvfs {
    russh_sftp::extensions::Statvfs {
        block_size: 4096,
        fragment_size,
        blocks: 1_000_000,
        blocks_free,
        blocks_avail,
        inodes: 0,
        inodes_free: 0,
        inodes_avail: 0,
        fs_id: 0,
        flags: 0,
        name_max: 255,
    }
}

#[test]
fn free_space_counts_the_blocks_an_ordinary_user_may_use() {
    assert_eq!(available_bytes(&statvfs(512, 100, 900)), 51_200);
}

#[test]
fn free_space_does_not_overflow() {
    assert_eq!(available_bytes(&statvfs(u64::MAX, 2, 2)), u64::MAX);
}

use super::*;

fn with(files: u64, folders: u64) -> Measured {
    Measured { files, folders, ..Measured::default() }
}

#[test]
fn a_size_under_one_kilobyte_is_plain_bytes() {
    assert_eq!(size_line(0), "0 B");
    assert_eq!(size_line(1023), "1023 B");
}

#[test]
fn a_larger_size_shows_the_short_form_and_the_exact_bytes() {
    assert_eq!(size_line(1024), "1.0 KB (1,024 bytes)");
    assert_eq!(size_line(4_521_983_744), "4.2 GB (4,521,983,744 bytes)");
}

#[test]
fn the_largest_size_does_not_overflow() {
    let text = size_line(u64::MAX);

    assert!(text.ends_with("(18,446,744,073,709,551,615 bytes)"), "{text}");
}

#[test]
fn contents_name_files_and_folders_with_singular_and_grouped_numbers() {
    assert_eq!(contents_line(&with(1_204, 87)), "1,204 files, 87 folders");
    assert_eq!(contents_line(&with(1, 1)), "1 file, 1 folder");
    assert_eq!(contents_line(&with(3, 0)), "3 files");
    assert_eq!(contents_line(&with(0, 2)), "2 folders");
    assert_eq!(contents_line(&with(0, 0)), "nothing inside");
}

#[test]
fn skipped_names_links_and_unreadable_folders_only_when_there_are_some() {
    assert_eq!(skipped_line(&with(5, 5)), None);
    assert_eq!(
        skipped_line(&Measured { symlinks: 2, unreadable: 1, ..Measured::default() }),
        Some("2 links not followed, 1 folder not readable".to_string())
    );
    assert_eq!(skipped_line(&Measured { symlinks: 1, ..Measured::default() }), Some("1 link not followed".to_string()));
    assert_eq!(
        skipped_line(&Measured { unreadable: 3, ..Measured::default() }),
        Some("3 folders not readable".to_string())
    );
}

#[test]
fn progress_says_how_much_has_been_seen_so_far() {
    assert_eq!(progress_line(&with(812, 40)), "measuring\u{2026} 812 files and 40 folders so far");
    assert_eq!(progress_line(&with(0, 0)), "measuring\u{2026}");
}

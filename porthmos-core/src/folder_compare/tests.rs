use porthmos_vfs::{Entry, VfsPath};

use super::*;

fn file(name: &str, size: u64, modified: Option<u64>) -> Entry {
    Entry {
        name: name.to_string(),
        path: VfsPath::from(format!("/{name}")),
        is_dir: false,
        size,
        modified,
        ..Default::default()
    }
}

fn folder(name: &str) -> Entry {
    Entry { is_dir: true, ..file(name, 0, None) }
}

#[test]
fn identical_folders_have_no_marks() {
    let left = vec![file("a", 5, Some(100)), folder("d")];
    let right = vec![file("a", 5, Some(100)), folder("d")];

    let marks = compare_folders(&left, &right);

    assert!(marks.is_identical());
    assert_eq!(marks.left("a"), None);
    assert_eq!(marks.right("d"), None);
}

#[test]
fn an_entry_on_one_side_only_is_marked_only_here() {
    let marks = compare_folders(&[file("l", 1, None)], &[file("r", 1, None)]);

    assert_eq!(marks.left("l"), Some(Mark::OnlyHere));
    assert_eq!(marks.right("r"), Some(Mark::OnlyHere));
    assert_eq!(marks.left("r"), None);
    assert_eq!(marks.right("l"), None);
}

#[test]
fn the_newer_copy_is_marked_newer_and_the_other_older() {
    let marks = compare_folders(&[file("a", 5, Some(1000))], &[file("a", 5, Some(500))]);

    assert_eq!(marks.left("a"), Some(Mark::Newer));
    assert_eq!(marks.right("a"), Some(Mark::Older));
}

#[test]
fn the_older_copy_on_the_left_is_marked_older() {
    let marks = compare_folders(&[file("a", 5, Some(500))], &[file("a", 5, Some(1000))]);

    assert_eq!(marks.left("a"), Some(Mark::Older));
    assert_eq!(marks.right("a"), Some(Mark::Newer));
}

#[test]
fn a_time_difference_inside_the_tolerance_is_not_a_difference() {
    let marks = compare_folders(&[file("a", 5, Some(100 + TIME_TOLERANCE_SECS))], &[file("a", 5, Some(100))]);

    assert!(marks.is_identical());
}

#[test]
fn a_time_difference_just_past_the_tolerance_is_marked() {
    let marks = compare_folders(&[file("a", 5, Some(101 + TIME_TOLERANCE_SECS))], &[file("a", 5, Some(100))]);

    assert_eq!(marks.left("a"), Some(Mark::Newer));
}

#[test]
fn the_same_time_with_another_size_is_marked_size_differs_on_both_sides() {
    let marks = compare_folders(&[file("a", 5, Some(100))], &[file("a", 9, Some(100))]);

    assert_eq!(marks.left("a"), Some(Mark::SizeDiffers));
    assert_eq!(marks.right("a"), Some(Mark::SizeDiffers));
}

#[test]
fn a_newer_copy_with_another_size_is_still_newer() {
    let marks = compare_folders(&[file("a", 9, Some(1000))], &[file("a", 5, Some(100))]);

    assert_eq!(marks.left("a"), Some(Mark::Newer));
}

#[test]
fn unknown_times_are_compared_by_size_only() {
    let same = compare_folders(&[file("a", 5, None)], &[file("a", 5, Some(100))]);
    let different = compare_folders(&[file("a", 5, None)], &[file("a", 6, Some(100))]);

    assert!(same.is_identical());
    assert_eq!(different.left("a"), Some(Mark::SizeDiffers));
}

#[test]
fn a_file_against_a_folder_of_the_same_name_is_a_kind_difference() {
    let marks = compare_folders(&[file("x", 5, Some(1))], &[folder("x")]);

    assert_eq!(marks.left("x"), Some(Mark::KindDiffers));
    assert_eq!(marks.right("x"), Some(Mark::KindDiffers));
}

#[test]
fn folders_with_the_same_name_are_never_marked_for_their_times_or_sizes() {
    let left = Entry { modified: Some(1000), ..folder("d") };
    let right = Entry { modified: Some(5), ..folder("d") };

    assert!(compare_folders(&[left], &[right]).is_identical());
}

#[test]
fn names_are_compared_exactly_including_case() {
    let marks = compare_folders(&[file("Readme", 1, None)], &[file("readme", 1, None)]);

    assert_eq!(marks.left("Readme"), Some(Mark::OnlyHere));
    assert_eq!(marks.right("readme"), Some(Mark::OnlyHere));
}

#[test]
fn the_summary_counts_each_kind_of_difference_once_per_pair() {
    let left = vec![
        file("only-left", 1, None),
        file("newer", 5, Some(1000)),
        file("sized", 1, Some(10)),
        file("same", 1, Some(10)),
    ];
    let right = vec![
        file("only-right", 1, None),
        file("newer", 5, Some(10)),
        file("sized", 2, Some(10)),
        file("same", 1, Some(10)),
    ];

    let marks = compare_folders(&left, &right);

    assert_eq!(marks.difference_count(), 4);
    assert!(!marks.is_identical());
}

#[test]
fn the_summary_says_when_nothing_differs() {
    assert_eq!(FolderMarks::default().summary(), "Comparing folders: they are identical");
}

#[test]
fn the_summary_counts_one_difference_in_the_singular() {
    let marks = compare_folders(&[file("a", 1, None)], &[]);

    assert_eq!(marks.summary(), "Comparing folders: 1 difference");
}

#[test]
fn the_summary_counts_several_differences_in_the_plural() {
    let marks = compare_folders(&[file("a", 1, None), file("b", 1, None)], &[]);

    assert_eq!(marks.summary(), "Comparing folders: 2 differences");
}

#[test]
fn the_server_needed_message_is_worded_by_core() {
    assert_eq!(needs_server_message(), "Connect to a server to compare folders");
}

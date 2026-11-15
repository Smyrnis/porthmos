use std::fs;
use std::path::PathBuf;

use porthmos_core::from_os_path;

use super::entries_from_paths;

#[test]
fn a_file_and_a_folder_become_entries_with_size_and_kind() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("notes.txt");
    let folder = root.path().join("photos");
    fs::write(&file, vec![0u8; 1234]).unwrap();
    fs::create_dir(&folder).unwrap();

    let (entries, problems) = entries_from_paths(&[file.clone(), folder.clone()]);

    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(entries.len(), 2);
    assert_eq!((entries[0].name.as_str(), entries[0].is_dir, entries[0].size), ("notes.txt", false, 1234));
    assert_eq!(entries[0].path, from_os_path(&file));
    assert!(entries[0].modified.is_some());
    assert_eq!((entries[1].name.as_str(), entries[1].is_dir), ("photos", true));
}

#[test]
fn a_hidden_name_is_marked_hidden() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".secret");
    fs::write(&file, b"x").unwrap();

    let (entries, _) = entries_from_paths(&[file]);

    assert!(entries[0].hidden);
}

#[test]
fn a_missing_path_is_reported_not_dropped_silently() {
    let root = tempfile::tempdir().unwrap();
    let present = root.path().join("here.txt");
    fs::write(&present, b"x").unwrap();
    let missing = root.path().join("gone.txt");

    let (entries, problems) = entries_from_paths(&[missing, present]);

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "here.txt");
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("gone.txt"), "{}", problems[0]);
}

#[test]
fn a_path_without_a_name_is_reported() {
    let (entries, problems) = entries_from_paths(&[PathBuf::from("/")]);

    assert!(entries.is_empty());
    assert_eq!(problems.len(), 1);
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_folder_is_a_folder_entry() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("real");
    let link = root.path().join("link");
    fs::create_dir(&folder).unwrap();
    std::os::unix::fs::symlink(&folder, &link).unwrap();

    let (entries, problems) = entries_from_paths(&[link]);

    assert!(problems.is_empty());
    assert_eq!((entries[0].name.as_str(), entries[0].is_dir), ("link", true));
}

#[cfg(unix)]
#[test]
fn a_broken_symlink_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let link = root.path().join("dangling");
    std::os::unix::fs::symlink(root.path().join("nowhere"), &link).unwrap();

    let (entries, problems) = entries_from_paths(&[link]);

    assert!(entries.is_empty());
    assert_eq!(problems.len(), 1);
}

#[test]
fn a_path_longer_than_260_characters_is_kept() {
    let root = tempfile::tempdir().unwrap();
    let mut deep = root.path().to_path_buf();
    for _ in 0..6 {
        deep.push("a-directory-name-that-is-fifty-characters-long-xxx");
    }
    if fs::create_dir_all(&deep).is_err() {
        return;
    }
    let file = deep.join("long-path-file.txt");
    fs::write(&file, b"x").unwrap();
    assert!(file.as_os_str().len() > 260);

    let (entries, problems) = entries_from_paths(std::slice::from_ref(&file));

    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(entries[0].path, from_os_path(&file));
}

#[test]
fn two_hundred_paths_keep_their_order() {
    let root = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = (0..200)
        .map(|n| {
            let path = root.path().join(format!("file-{:03}.txt", 199 - n));
            fs::write(&path, b"x").unwrap();
            path
        })
        .collect();

    let (entries, problems) = entries_from_paths(&paths);

    assert!(problems.is_empty());
    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    let expected: Vec<String> = (0..200).map(|n| format!("file-{:03}.txt", 199 - n)).collect();
    assert_eq!(names, expected.iter().map(String::as_str).collect::<Vec<_>>());
}

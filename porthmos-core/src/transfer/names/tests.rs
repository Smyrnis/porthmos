use porthmos_vfs::{NameRules, VfsPath};

use super::*;
use crate::transfer::plan::PlannedFile;

fn file(destination: &str) -> PlannedFile {
    PlannedFile {
        source: VfsPath::new("/src").join(VfsPath::new(destination).file_name().unwrap().to_string_lossy().as_ref()),
        destination: VfsPath::new(destination),
        display_name: destination.to_string(),
        size: 1,
        existing: None,
        source_modified: None,
        partial: None,
        resume: false,
    }
}

fn sanitize_under(root: &VfsPath, path: &VfsPath, rules: NameRules) -> VfsPath {
    Renames::default().sanitize_under(root, path, rules)
}

fn destinations(files: &[PlannedFile]) -> Vec<String> {
    files.iter().map(|file| file.destination.to_string_lossy().into_owned()).collect()
}

#[test]
fn sanitize_under_keeps_the_root_and_cleans_what_follows() {
    let root = VfsPath::new("/C:/Users/me");
    let path = VfsPath::new("/C:/Users/me/a:b/CON.txt");

    assert_eq!(sanitize_under(&root, &path, NameRules::Windows), VfsPath::new("/C:/Users/me/a_b/_CON.txt"));
    assert_eq!(sanitize_under(&root, &path, NameRules::Permissive), path);
}

#[test]
fn a_path_outside_the_root_is_left_alone() {
    let path = VfsPath::new("/other/a:b");

    assert_eq!(sanitize_under(&VfsPath::new("/C:/x"), &path, NameRules::Windows), path);
}

#[test]
fn every_changed_name_is_recorded_once_per_place() {
    let root = VfsPath::new("/C:/d");
    let mut renames = Renames::default();

    renames.sanitize_under(&root, &VfsPath::new("/C:/d/a:b/f?.txt"), NameRules::Windows);
    renames.sanitize_under(&root, &VfsPath::new("/C:/d/a:b/g.txt"), NameRules::Windows);
    renames.sanitize_under(&root, &VfsPath::new("/C:/d/fine/f?.txt"), NameRules::Windows);

    let pairs: Vec<(String, String)> = renames.into_list().into_iter().map(|r| (r.from, r.to)).collect();
    assert_eq!(
        pairs,
        [("a:b", "a_b"), ("f?.txt", "f_.txt"), ("f?.txt", "f_.txt")]
            .map(|(from, to)| (from.to_string(), to.to_string()))
    );
}

#[test]
fn dedupe_names_renames_later_names_that_fold_the_same() {
    let mut files = vec![file("/d/README.md"), file("/d/Readme.md"), file("/d/readme.MD")];

    let renamed = dedupe_names(&mut files, &[], NameRules::Windows);

    assert_eq!(destinations(&files), ["/d/README.md", "/d/Readme (1).md", "/d/readme (2).MD"]);
    assert_eq!(files[1].display_name, "/d/Readme.md");
    assert_eq!(renamed.len(), 2);
}

#[test]
fn dedupe_names_changes_nothing_on_a_case_sensitive_destination() {
    let mut files = vec![file("/d/README.md"), file("/d/Readme.md"), file("/d/readme.MD")];

    let renamed = dedupe_names(&mut files, &[], NameRules::Permissive);

    assert_eq!(destinations(&files), ["/d/README.md", "/d/Readme.md", "/d/readme.MD"]);
    assert!(renamed.is_empty());
}

#[test]
fn the_same_name_in_different_folders_is_untouched() {
    let mut files = vec![file("/d/one/a.txt"), file("/d/two/A.txt")];

    dedupe_names(&mut files, &[], NameRules::Windows);

    assert_eq!(destinations(&files), ["/d/one/a.txt", "/d/two/A.txt"]);
}

#[test]
fn the_notice_counts_the_names_and_gives_one_example() {
    let renamed = vec![
        Renamed { from: "a:b.txt".into(), to: "a_b.txt".into() },
        Renamed { from: "CON".into(), to: "_CON".into() },
    ];

    assert_eq!(
        renamed_notice(&renamed),
        "Renamed 2 names the destination can't store, e.g. \"a:b.txt\" to \"a_b.txt\""
    );
    assert_eq!(
        renamed_notice(&renamed[..1]),
        "Renamed 1 name the destination can't store, e.g. \"a:b.txt\" to \"a_b.txt\""
    );
}

#[test]
fn dedupe_names_keeps_a_file_away_from_a_planned_folder_of_the_same_name() {
    let mut files = vec![file("/d/a_b")];

    let renamed = dedupe_names(&mut files, &[VfsPath::new("/d/A_B")], NameRules::Windows);

    assert_eq!(destinations(&files), ["/d/a_b (1)"]);
    assert_eq!(renamed.len(), 1);
}

#[test]
fn folders_that_differ_only_in_case_get_the_first_spelling() {
    let root = VfsPath::new("/d");
    let mut renames = Renames::default();

    let first = renames.sanitize_folder(&root, &VfsPath::new("/d/p/Docs"), NameRules::Windows);
    let second = renames.sanitize_folder(&root, &VfsPath::new("/d/p/docs"), NameRules::Windows);
    let inside = renames.sanitize_under(&root, &VfsPath::new("/d/P/DOCS/a.txt"), NameRules::Windows);

    assert_eq!(first, VfsPath::new("/d/p/Docs"));
    assert_eq!(second, VfsPath::new("/d/p/Docs"));
    assert_eq!(inside, VfsPath::new("/d/p/Docs/a.txt"));
}

#[test]
fn folder_spellings_are_untouched_on_a_case_sensitive_destination() {
    let root = VfsPath::new("/d");
    let mut renames = Renames::default();

    renames.sanitize_folder(&root, &VfsPath::new("/d/Docs"), NameRules::Permissive);
    let second = renames.sanitize_folder(&root, &VfsPath::new("/d/docs"), NameRules::Permissive);

    assert_eq!(second, VfsPath::new("/d/docs"));
}

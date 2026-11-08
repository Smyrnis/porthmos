use chrono::{DateTime, TimeZone, Utc};

use super::*;
use crate::Paths;

fn paths() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    (dir, paths)
}

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 22, hour, 0, 0).unwrap()
}

#[test]
fn a_missing_file_loads_empty_and_quiet() {
    let (_dir, paths) = paths();

    let (last_used, warning) = LastUsed::load(&paths);

    assert!(warning.is_none());
    assert!(last_used.times().is_empty());
    assert!(!paths.last_used_file().exists());
}

#[test]
fn a_recorded_time_is_returned_for_that_connection_only() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);

    last_used.record("web", at(9));

    assert_eq!(last_used.get("web"), Some(at(9)));
    assert_eq!(last_used.get("db"), None);
}

#[test]
fn recording_again_keeps_only_the_newest_time() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);

    last_used.record("web", at(9));
    last_used.record("web", at(11));

    assert_eq!(last_used.get("web"), Some(at(11)));
    assert_eq!(last_used.times().len(), 1);
}

#[test]
fn a_rendered_file_loads_back_with_every_time() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("web", at(9));
    last_used.record("db prod", at(10));
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), last_used.render().unwrap()).unwrap();

    let (loaded, warning) = LastUsed::load(&paths);

    assert!(warning.is_none());
    assert_eq!(loaded.get("web"), Some(at(9)));
    assert_eq!(loaded.get("db prod"), Some(at(10)));
}

#[test]
fn renaming_moves_the_time_to_the_new_name() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("web", at(9));

    assert!(last_used.rename("web", "web-old"));

    assert_eq!(last_used.get("web"), None);
    assert_eq!(last_used.get("web-old"), Some(at(9)));
}

#[test]
fn renaming_a_connection_that_was_never_used_changes_nothing() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("db", at(9));

    assert!(!last_used.rename("web", "web-old"));
    assert!(!last_used.rename("db", "db"));

    assert_eq!(last_used.times().len(), 1);
    assert_eq!(last_used.get("db"), Some(at(9)));
}

#[test]
fn renaming_over_a_name_that_has_a_time_replaces_it() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("web", at(9));
    last_used.record("old", at(7));

    assert!(last_used.rename("web", "old"));

    assert_eq!(last_used.get("old"), Some(at(9)));
    assert_eq!(last_used.times().len(), 1);
}

#[test]
fn forgetting_removes_the_time() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("web", at(9));

    assert!(last_used.forget("web"));
    assert!(!last_used.forget("web"));

    assert_eq!(last_used.get("web"), None);
}

#[test]
fn a_broken_file_is_set_aside_with_a_warning_and_loads_empty() {
    let (_dir, paths) = paths();
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), "not toml at all [").unwrap();

    let (last_used, warning) = LastUsed::load(&paths);

    assert!(last_used.times().is_empty());
    assert!(last_used.is_writable());
    assert!(warning.unwrap().contains("last_used.toml.broken"));
    assert_eq!(
        std::fs::read_to_string(paths.last_used_file().with_extension("toml.broken")).unwrap(),
        "not toml at all ["
    );
}

#[test]
fn a_time_that_is_not_a_date_breaks_the_whole_file() {
    let (_dir, paths) = paths();
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), "[last_used]\nweb = \"yesterday\"\n").unwrap();

    let (last_used, warning) = LastUsed::load(&paths);

    assert!(last_used.times().is_empty());
    assert!(warning.is_some());
}

#[test]
fn unknown_keys_in_the_file_are_ignored() {
    let (_dir, paths) = paths();
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), "future = 1\n[last_used]\nweb = \"2026-10-22T09:00:00Z\"\n").unwrap();

    let (last_used, warning) = LastUsed::load(&paths);

    assert!(warning.is_none());
    assert_eq!(last_used.get("web"), Some(at(9)));
}

#[test]
fn the_rendered_file_holds_only_names_and_times() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("web", at(9));

    let text = last_used.render().unwrap();

    assert_eq!(text.trim(), "[last_used]\nweb = \"2026-10-22T09:00:00Z\"");
}

#[test]
fn the_file_lives_in_the_state_folder() {
    let root = std::path::Path::new("/r");
    assert_eq!(Paths::in_dir(root).last_used_file(), root.join("state").join("last_used.toml"));
}

#[test]
fn renaming_from_a_connection_never_used_clears_a_stale_time_at_the_target() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("web", at(9));

    assert!(last_used.rename("staging", "web"));

    assert_eq!(last_used.get("web"), None);
    assert!(last_used.times().is_empty());
}

#[test]
fn renaming_between_two_names_that_have_no_time_changes_nothing() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);

    assert!(!last_used.rename("staging", "web"));
}

#[test]
fn merging_moves_the_time_when_the_target_has_none() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("old", at(9));

    assert!(last_used.merge("old", "new"));

    assert_eq!(last_used.get("new"), Some(at(9)));
    assert_eq!(last_used.get("old"), None);
}

#[test]
fn merging_keeps_the_later_of_the_two_times() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("old", at(9));
    last_used.record("new", at(11));
    last_used.record("older", at(12));
    last_used.record("newer", at(8));

    assert!(last_used.merge("old", "new"));
    assert!(last_used.merge("older", "newer"));

    assert_eq!(last_used.get("new"), Some(at(11)));
    assert_eq!(last_used.get("newer"), Some(at(12)));
    assert_eq!(last_used.times().len(), 2);
}

#[test]
fn merging_a_connection_never_used_or_onto_itself_changes_nothing() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("new", at(9));

    assert!(!last_used.merge("old", "new"));
    assert!(!last_used.merge("new", "new"));

    assert_eq!(last_used.get("new"), Some(at(9)));
}

fn folder(path: &str) -> VfsPath {
    VfsPath::new(path)
}

#[test]
fn a_recorded_folder_is_returned_for_that_connection_only() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);

    assert!(last_used.record_folder("web", &folder("/srv/www")));

    assert_eq!(last_used.folder("web"), Some(folder("/srv/www")));
    assert_eq!(last_used.folder("db"), None);
}

#[test]
fn recording_the_same_folder_again_changes_nothing() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record_folder("web", &folder("/srv/www"));

    assert!(!last_used.record_folder("web", &folder("/srv/www")));
    assert!(last_used.record_folder("web", &folder("/srv")));

    assert_eq!(last_used.folder("web"), Some(folder("/srv")));
}

#[test]
fn folders_are_written_in_their_own_table_and_load_back() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("web", at(9));
    last_used.record_folder("web", &folder("/srv/it's here"));
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    let text = last_used.render().unwrap();
    std::fs::write(paths.last_used_file(), &text).unwrap();

    let (loaded, warning) = LastUsed::load(&paths);

    assert!(warning.is_none());
    assert!(text.contains("[last_folder]"), "{text}");
    assert_eq!(loaded.folder("web"), Some(folder("/srv/it's here")));
    assert_eq!(loaded.get("web"), Some(at(9)));
}

#[test]
fn a_file_from_before_folders_existed_loads_without_any() {
    let (_dir, paths) = paths();
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), "[last_used]\nweb = \"2026-10-22T09:00:00Z\"\n").unwrap();

    let (loaded, warning) = LastUsed::load(&paths);

    assert!(warning.is_none());
    assert_eq!(loaded.folder("web"), None);
    assert_eq!(loaded.get("web"), Some(at(9)));
}

#[test]
fn renaming_carries_the_folder_with_the_time() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record_folder("web", &folder("/srv"));

    assert!(last_used.rename("web", "web-old"));

    assert_eq!(last_used.folder("web"), None);
    assert_eq!(last_used.folder("web-old"), Some(folder("/srv")));
}

#[test]
fn renaming_a_connection_with_no_folder_clears_the_targets_old_folder() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record_folder("web", &folder("/srv"));

    assert!(last_used.rename("staging", "web"));

    assert_eq!(last_used.folder("web"), None);
}

#[test]
fn forgetting_removes_the_folder_too() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record_folder("web", &folder("/srv"));

    assert!(last_used.forget("web"));

    assert_eq!(last_used.folder("web"), None);
}

#[test]
fn merging_keeps_the_folder_of_whichever_connection_was_used_last() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record("old", at(9));
    last_used.record_folder("old", &folder("/from-old"));
    last_used.record("new", at(11));
    last_used.record_folder("new", &folder("/from-new"));
    last_used.record("older", at(12));
    last_used.record_folder("older", &folder("/from-older"));
    last_used.record("newer", at(8));
    last_used.record_folder("newer", &folder("/from-newer"));

    assert!(last_used.merge("old", "new"));
    assert!(last_used.merge("older", "newer"));

    assert_eq!(last_used.folder("new"), Some(folder("/from-new")));
    assert_eq!(last_used.folder("newer"), Some(folder("/from-older")));
    assert_eq!(last_used.folder("old"), None);
    assert_eq!(last_used.folder("older"), None);
}

#[test]
fn merging_moves_a_folder_when_the_target_has_none() {
    let (_dir, paths) = paths();
    let (mut last_used, _) = LastUsed::load(&paths);
    last_used.record_folder("old", &folder("/x"));

    assert!(last_used.merge("old", "new"));

    assert_eq!(last_used.folder("new"), Some(folder("/x")));
}

use std::path::Path;

use porthmos_vfs::Environment;

use super::*;

fn touch(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "x").unwrap();
}

fn env_with_home(home: &Path) -> Environment {
    Environment { home: Some(home.to_path_buf()), ..Environment::default() }
}

#[test]
fn filezilla_is_found_in_the_config_folder() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_home(home.path());
    assert_eq!(default_location(ImportSource::FileZilla, &env, None), None);
    let file = home.path().join(".config/filezilla/sitemanager.xml");
    touch(&file);
    assert_eq!(default_location(ImportSource::FileZilla, &env, None), Some(file));
}

#[test]
fn filezilla_is_found_in_app_data_on_windows_layouts() {
    let app_data = tempfile::tempdir().unwrap();
    let file = app_data.path().join("FileZilla/sitemanager.xml");
    touch(&file);
    assert_eq!(default_location(ImportSource::FileZilla, &Environment::default(), Some(app_data.path())), Some(file));
}

#[test]
fn winscp_is_found_only_in_app_data() {
    let app_data = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    touch(&home.path().join("WinSCP.ini"));
    assert_eq!(default_location(ImportSource::WinScp, &env_with_home(home.path()), Some(app_data.path())), None);
    let file = app_data.path().join("WinSCP.ini");
    touch(&file);
    assert_eq!(default_location(ImportSource::WinScp, &Environment::default(), Some(app_data.path())), Some(file));
}

#[test]
fn putty_folders_follow_putty_own_lookup_order() {
    let home = Path::new("/home/a");
    let folders = putty_session_folders(Some(home), Some("/custom".into()), Some("/xdg".into()));
    assert_eq!(
        folders,
        [
            Path::new("/custom").to_path_buf(),
            Path::new("/xdg/putty/sessions").to_path_buf(),
            home.join(".config/putty/sessions"),
            home.join(".putty/sessions"),
        ]
    );
    assert_eq!(
        putty_session_folders(Some(home), None, None),
        [home.join(".config/putty/sessions"), home.join(".putty/sessions")]
    );
    assert!(putty_session_folders(None, None, None).is_empty());
}

#[test]
fn putty_default_is_the_first_existing_folder() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_home(home.path());
    assert_eq!(default_location(ImportSource::Putty, &env, None), None);
    let legacy = home.path().join(".putty/sessions");
    std::fs::create_dir_all(&legacy).unwrap();
    assert_eq!(default_location(ImportSource::Putty, &env, None), Some(legacy));
}

#[test]
fn a_folder_where_a_file_is_expected_is_not_offered() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".config/filezilla/sitemanager.xml")).unwrap();
    assert_eq!(default_location(ImportSource::FileZilla, &env_with_home(home.path()), None), None);
}

#[test]
fn a_typed_path_is_trimmed_and_the_home_marker_expanded() {
    let home = Path::new("/home/a");
    assert_eq!(path_from_text("  /srv/x.xml \n", Some(home)), Path::new("/srv/x.xml"));
    assert_eq!(path_from_text("~/sites/x.xml", Some(home)), home.join("sites/x.xml"));
    assert_eq!(path_from_text("~", Some(home)), home);
    assert_eq!(path_from_text("~/sites", None), Path::new("~/sites"));
    assert_eq!(path_from_text("~other/x", Some(home)), Path::new("~other/x"));
    assert_eq!(path_from_text("   ", Some(home)), Path::new(""));
}

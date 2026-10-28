use porthmos_vfs::FileKind;

use super::*;

#[test]
fn bits_map_to_drive_letters_from_a() {
    assert_eq!(drive_letters(0b101), vec!['A', 'C']);
    assert_eq!(drive_letters(0), Vec::<char>::new());
    assert_eq!(drive_letters(1 << 25), vec!['Z']);
    assert_eq!(drive_letters(1 << 26), Vec::<char>::new());
}

#[test]
fn drives_become_folder_entries_under_the_root() {
    let entries = entries_for(&['C', 'D']);

    let shown: Vec<(&str, VfsPath, bool)> =
        entries.iter().map(|entry| (entry.name.as_str(), entry.path.clone(), entry.is_dir)).collect();
    assert_eq!(shown, [("C:", VfsPath::new("/C:"), true), ("D:", VfsPath::new("/D:"), true)]);
    assert!(entries.iter().all(|entry| entry.size == 0 && entry.modified.is_none() && !entry.hidden));
}

#[test]
fn drives_become_directory_items_under_the_root() {
    let items = items_for(&['C']);

    assert_eq!((items[0].name.as_str(), items[0].path.clone()), ("C:", VfsPath::new("/C:")));
    assert_eq!(items[0].metadata.kind, FileKind::Dir);
}

#[test]
fn the_root_itself_is_a_folder_with_no_times() {
    let metadata = root_metadata();

    assert_eq!((metadata.kind, metadata.size, metadata.modified, metadata.permissions), (FileKind::Dir, 0, None, None));
}

#[cfg(unix)]
#[test]
fn on_unix_the_root_is_an_ordinary_folder() {
    assert!(!lists_drives(&VfsPath::root()));
}

#[cfg(windows)]
#[test]
fn only_the_bare_root_lists_drives() {
    assert!(lists_drives(&VfsPath::root()));
    assert!(!lists_drives(&VfsPath::new("/C:")));
    assert!(!lists_drives(&VfsPath::new("/C:/x")));
    assert!(!lists_drives(&VfsPath::new("relative")));
}

#[cfg(windows)]
mod local {
    use porthmos_vfs::FileSystem;

    use super::*;
    use crate::LocalFs;

    fn local() -> LocalFs {
        LocalFs::new(std::env::temp_dir())
    }

    #[tokio::test]
    async fn the_root_lists_the_drives_and_each_is_a_folder() {
        let entries = local().list(&VfsPath::root()).await.unwrap();

        assert!(entries.iter().any(|entry| entry.name == "C:" && entry.is_dir));
        assert!(entries.iter().all(|entry| entry.path == VfsPath::root().join(&entry.name)));
    }

    #[tokio::test]
    async fn the_root_reads_as_directory_items_and_stats_as_a_folder() {
        let local = local();

        let items = local.read_dir(&VfsPath::root()).await.unwrap();
        let metadata = local.stat(&VfsPath::root()).await.unwrap();

        assert!(items.iter().any(|item| item.name == "C:"));
        assert_eq!(metadata.kind, FileKind::Dir);
    }

    #[tokio::test]
    async fn a_drive_lists_like_any_folder_and_goes_back_to_the_root() {
        let entries = local().list(&VfsPath::new("/C:")).await.unwrap();

        assert!(!entries.is_empty());
        assert_eq!(VfsPath::new("/C:").parent(), Some(VfsPath::root()));
    }

    #[tokio::test]
    async fn the_root_has_no_free_space_figure() {
        assert_eq!(local().free_space(&VfsPath::root()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn creating_something_in_the_root_is_refused_with_the_drive_message() {
        let error = local().create_dir(&VfsPath::new("/new")).await.unwrap_err();

        assert!(error.to_string().contains("drive"), "{error}");
    }
}

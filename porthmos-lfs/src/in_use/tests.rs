use super::*;

#[test]
fn sharing_and_lock_violations_are_in_use() {
    for code in [32, 33] {
        let text = in_use_message(Some(code)).unwrap();
        assert!(text.contains("in use by another program"), "{text}");
    }
    assert_eq!(in_use_message(Some(5)), None);
    assert_eq!(in_use_message(Some(2)), None);
    assert_eq!(in_use_message(None), None);
}

#[cfg(unix)]
#[test]
fn on_unix_an_error_is_passed_on_unchanged() {
    let error = explain(io::Error::from_raw_os_error(32));

    assert!(!error.to_string().contains("in use by another program"), "{error}");
}

#[cfg(windows)]
mod local {
    use std::{fs, os::windows::fs::OpenOptionsExt};

    use porthmos_vfs::FileSystem;

    use crate::{LocalFs, from_os_path};

    const CONTENT: &str = "kept";

    fn held_file() -> (tempfile::TempDir, std::path::PathBuf, fs::File) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("held.txt");
        fs::write(&path, CONTENT).unwrap();
        let holder = fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
        (dir, path, holder)
    }

    fn local() -> LocalFs {
        LocalFs::new(std::env::temp_dir())
    }

    #[tokio::test]
    async fn a_file_held_open_cannot_be_deleted_and_says_why() {
        let (_dir, path, holder) = held_file();

        let error = local().delete(&from_os_path(&path)).await.unwrap_err();
        drop(holder);

        assert!(error.to_string().contains("in use by another program"), "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), CONTENT);
    }

    #[tokio::test]
    async fn a_file_held_open_cannot_be_renamed_and_says_why() {
        let (dir, path, holder) = held_file();
        let target = dir.path().join("renamed.txt");

        let error = local().rename(&from_os_path(&path), &from_os_path(&target)).await.unwrap_err();
        drop(holder);

        assert!(error.to_string().contains("in use by another program"), "{error}");
        assert!(path.exists() && !target.exists());
    }

    #[tokio::test]
    async fn a_file_held_open_cannot_be_written_and_says_why() {
        let (_dir, path, holder) = held_file();

        let error = match local().open_write(&from_os_path(&path), 0).await {
            Ok(_) => panic!("a held file was opened for writing"),
            Err(error) => error,
        };
        drop(holder);

        assert!(error.to_string().contains("in use by another program"), "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), CONTENT);
    }
}

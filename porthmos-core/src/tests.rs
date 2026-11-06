use std::path::Path;

use super::*;

#[cfg(all(feature = "sftp", feature = "ftp", feature = "webdav", feature = "s3", feature = "scp"))]
#[test]
fn builtin_protocols_offer_every_protocol_in_order() {
    let protocols = builtin_protocols(&Paths::in_dir(Path::new("/tmp/t")));

    assert_eq!(
        protocols.iter().map(|protocol| protocol.id()).collect::<Vec<_>>(),
        vec!["sftp", "ftp", "webdav", "s3", "scp"]
    );
}

#[cfg(not(any(feature = "sftp", feature = "ftp", feature = "webdav", feature = "s3", feature = "scp")))]
#[test]
fn without_protocol_features_no_protocol_is_built_in() {
    assert!(builtin_protocols(&Paths::in_dir(Path::new("/tmp/t"))).is_empty());
}

#[cfg(unix)]
#[test]
fn core_dumps_can_be_switched_off() {
    disable_core_dumps();

    let mut limit = libc::rlimit { rlim_cur: 1, rlim_max: 1 };
    assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) }, 0);
    assert_eq!(limit.rlim_cur, 0);
}

#[tokio::test]
async fn a_broken_history_file_is_set_aside_with_a_warning_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.history_file(), "not toml [").unwrap();

    let (_core, mut events) = Core::builder().without_keyring().paths(paths.clone()).start().unwrap();

    let warning = loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await;
        match event.unwrap().unwrap() {
            Event::Notice { severity: Severity::Warning, message } if message.to_string().contains("history") => {
                break message;
            }
            _ => {}
        }
    };
    assert!(warning.to_string().contains("history.toml.broken"), "{warning}");
    assert!(paths.history_file().with_extension("toml.broken").exists());
}

#[tokio::test]
async fn saved_history_is_loaded_at_start_and_listed_on_request() {
    use history::testing::SaveNow;

    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    let mut saved = history::History::load(&paths).0;
    saved.record(history::testing::sample("kept.txt", history::HistoryResult::Done)).unwrap();

    let (core, mut events) = Core::builder().without_keyring().paths(paths).start().unwrap();
    core.send(Command::ListHistory);

    let listed = loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await;
        if let Event::History(entries) = event.unwrap().unwrap() {
            break entries;
        }
    };
    assert_eq!(listed.iter().map(|entry| entry.label.as_str()).collect::<Vec<_>>(), ["kept.txt"]);
}

#[tokio::test]
async fn saved_last_used_times_are_published_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), "[last_used]\nweb = \"2026-10-22T09:00:00Z\"\n").unwrap();

    let (_core, mut events) = Core::builder().without_keyring().paths(paths).start().unwrap();

    let published = loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await;
        if let Event::LastUsed(times) = event.unwrap().unwrap() {
            break times;
        }
    };
    assert_eq!(published.len(), 1);
    assert_eq!(published["web"], chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 10, 22, 9, 0, 0).unwrap());
}

#[tokio::test]
async fn a_broken_last_used_file_is_set_aside_with_a_warning_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.last_used_file(), "not toml [").unwrap();

    let (_core, mut events) = Core::builder().without_keyring().paths(paths.clone()).start().unwrap();

    let warning = loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await;
        match event.unwrap().unwrap() {
            Event::Notice { severity: Severity::Warning, message } if message.to_string().contains("last-used") => {
                break message;
            }
            _ => {}
        }
    };
    assert!(warning.to_string().contains("last_used.toml.broken"), "{warning}");
    assert!(paths.last_used_file().with_extension("toml.broken").exists());
}

#[tokio::test]
async fn the_history_warning_comes_before_the_bookmark_warnings() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(paths.history_file(), "not toml [").unwrap();
    std::fs::write(paths.bookmarks_file(), "not toml [").unwrap();

    let (_core, mut events) = Core::builder().without_keyring().paths(paths).start().unwrap();

    let first = loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv()).await;
        if let Event::Notice { message, .. } = event.unwrap().unwrap() {
            break message;
        }
    };
    assert!(first.to_string().contains("history"), "{first}");
}

#[tokio::test]
async fn shutting_down_a_running_core_waits_for_it_and_can_be_repeated() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _events) = Core::builder().without_keyring().paths(Paths::in_dir(dir.path())).start().unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(3), core.shutdown()).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_millis(500), core.shutdown()).await.unwrap();
}

#[tokio::test]
async fn shutting_down_a_detached_handle_returns_at_once() {
    let (core, _commands) = CoreHandle::detached();

    tokio::time::timeout(std::time::Duration::from_millis(500), core.shutdown()).await.unwrap();
}

#[cfg(any(feature = "sftp", feature = "ftp", feature = "webdav", feature = "s3", feature = "scp"))]
#[test]
fn every_builtin_protocol_has_a_description_and_only_s3_notes_limits() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());

    let protocols = builtin_protocols(&paths);

    assert!(!protocols.is_empty());
    for protocol in &protocols {
        let info = ProtocolInfo::from_protocol(protocol.as_ref());
        assert!(!info.description.is_empty(), "{} has no description", info.id);
        let expects_note = info.id == "s3";
        assert_eq!(info.limits_note.is_some(), expects_note, "{}", info.id);
    }
    #[cfg(feature = "s3")]
    {
        let s3 = protocols.iter().find(|protocol| protocol.id() == "s3").map(|protocol| protocol.limits_note());
        assert_eq!(s3, Some(Some("no permissions or symlinks \u{b7} rename is copy + delete")));
    }
}

fn state_with_local_folder(folder: &Path) -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.state_file(), format!("local_folder = {:?}\n", from_os_path(folder).to_string_lossy()))
        .unwrap();
    (dir, paths)
}

#[tokio::test]
async fn a_saved_local_folder_that_still_exists_is_offered_at_start() {
    let folder = tempfile::tempdir().unwrap();
    let (_dir, paths) = state_with_local_folder(folder.path());

    let (core, _events) = Core::builder().without_keyring().paths(paths).start().unwrap();

    assert_eq!(core.remembered_local_folder(), Some(from_os_path(folder.path())));
}

#[tokio::test]
async fn a_saved_local_folder_that_is_gone_is_not_offered() {
    let folder = tempfile::tempdir().unwrap();
    let gone = folder.path().join("deleted");
    let (_dir, paths) = state_with_local_folder(&gone);

    let (core, _events) = Core::builder().without_keyring().paths(paths).start().unwrap();

    assert_eq!(core.remembered_local_folder(), None);
}

#[tokio::test]
async fn a_saved_local_folder_that_is_a_file_is_not_offered() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("file.txt");
    std::fs::write(&file, b"x").unwrap();
    let (_dir, paths) = state_with_local_folder(&file);

    let (core, _events) = Core::builder().without_keyring().paths(paths).start().unwrap();

    assert_eq!(core.remembered_local_folder(), None);
}

#[tokio::test]
async fn nothing_is_offered_when_nothing_was_saved() {
    let dir = tempfile::tempdir().unwrap();

    let (core, _events) = Core::builder().without_keyring().paths(Paths::in_dir(dir.path())).start().unwrap();

    assert_eq!(core.remembered_local_folder(), None);
}

fn size_descending_hidden() -> crate::config::settings::PanelSettings {
    use crate::listing::{SortKey, SortOrder, SortSpec};
    crate::config::settings::PanelSettings {
        show_hidden: true,
        sort: SortSpec { key: SortKey::Size, order: SortOrder::Descending },
    }
}

#[tokio::test]
async fn the_panels_start_with_the_configured_defaults_when_nothing_was_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings { panel: size_descending_hidden(), ..Settings::default() };

    let (core, _events) =
        Core::builder().without_keyring().paths(Paths::in_dir(dir.path())).settings(settings).start().unwrap();

    assert_eq!(core.panel_settings(PanelKind::Local), size_descending_hidden());
    assert_eq!(core.panel_settings(PanelKind::Remote), size_descending_hidden());
}

#[tokio::test]
async fn a_panels_remembered_choices_win_over_the_configured_defaults_for_that_panel_only() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::in_dir(dir.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    std::fs::write(paths.state_file(), "[remote_panel]\nshow_hidden = false\nsort_key = \"name\"\n").unwrap();
    let settings = Settings { panel: size_descending_hidden(), ..Settings::default() };

    let (core, _events) = Core::builder().without_keyring().paths(paths).settings(settings).start().unwrap();

    assert_eq!(core.panel_settings(PanelKind::Local), size_descending_hidden());
    let remote = core.panel_settings(PanelKind::Remote);
    assert!(!remote.show_hidden);
    assert_eq!(remote.sort.key, crate::listing::SortKey::Name);
    assert_eq!(remote.sort.order, crate::listing::SortOrder::Descending);
}

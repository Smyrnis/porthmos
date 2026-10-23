use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;
use crate::engine::{Command, testing::test_engine};

#[tokio::test]
async fn listing_the_local_side_sends_its_entries() {
    let mut t = test_engine();
    std::fs::write(t.dir.path().join("file.txt"), b"x").unwrap();
    let dir = t.dir.path().to_path_buf();

    t.engine.handle_command(Command::List { location: Location::Local, path: Some(dir.clone()) });

    match t.next_event().await {
        Event::Listed { location: Location::Local, path, entries } => {
            assert_eq!(path, dir);
            assert_eq!(entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["file.txt"]);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_local_listing_failure_shows_the_bare_error() {
    let mut t = test_engine();

    t.engine
        .handle_command(Command::List { location: Location::Local, path: Some(PathBuf::from("/definitely/missing")) });

    match t.next_event().await {
        Event::Notice { severity: Severity::Error, message } => {
            assert_eq!(message, "No such file or directory (os error 2)");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_remote_listing_failure_names_the_folder() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");

    t.engine.handle_command(Command::List { location: Location::Session(session), path: Some(PathBuf::from("/gone")) });

    match t.next_event().await {
        Event::Notice { severity: Severity::Error, message } => assert!(message.starts_with("Unable to list /gone:\n")),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_listing_without_a_path_uses_the_home_folder() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/a", b"", None);

    t.engine.handle_command(Command::List { location: Location::Session(session), path: None });

    assert!(matches!(t.next_event().await, Event::Listed { path, .. } if path == std::path::Path::new("/home/user")));
}

#[test]
fn operations_on_a_vanished_session_are_dropped() {
    let mut t = test_engine();

    t.engine.handle_command(Command::List { location: Location::Session(42), path: None });
    t.engine.handle_command(Command::CreateDir { location: Location::Session(42), path: PathBuf::from("/x") });

    assert!(t.drain().is_empty());
}

#[tokio::test]
async fn mkdir_creates_the_directory_and_reports_the_change() {
    let mut t = test_engine();
    let target = t.dir.path().join("new_dir");

    t.engine.handle_command(Command::CreateDir { location: Location::Local, path: target.clone() });

    assert!(matches!(t.next_event().await, Event::LocationChanged { location: Location::Local }));
    assert!(target.is_dir());
}

#[tokio::test]
async fn a_failed_mkdir_shows_an_error_and_does_not_report_a_change() {
    let mut t = test_engine();
    let target = t.dir.path().join("dup");
    std::fs::create_dir(&target).unwrap();

    t.engine.handle_command(Command::CreateDir { location: Location::Local, path: target });

    assert!(matches!(t.next_event().await, Event::Notice { severity: Severity::Error, .. }));
    tokio::task::yield_now().await;
    assert!(t.drain().is_empty());
}

#[tokio::test]
async fn making_a_folder_that_exists_names_it() {
    let mut t = test_engine();
    let target = t.dir.path().join("dup");
    std::fs::create_dir(&target).unwrap();

    t.engine.handle_command(Command::CreateDir { location: Location::Local, path: target });

    match t.next_event().await {
        Event::Notice { severity: Severity::Error, message } => assert_eq!(message, "\"dup\" already exists"),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn making_a_remote_folder_that_exists_names_it() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/dup");

    t.engine.handle_command(Command::CreateDir { location: Location::Session(session), path: PathBuf::from("/dup") });

    match t.next_event().await {
        Event::Notice { message, .. } => assert_eq!(message, "Unable to create directory:\n\"dup\" already exists"),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_failed_remote_mkdir_says_what_failed() {
    let mut t = test_engine();
    let (session, _) = t.add_session("srv");

    t.engine.handle_command(Command::CreateDir {
        location: Location::Session(session),
        path: PathBuf::from("/no/parent/x"),
    });

    match t.next_event().await {
        Event::Notice { message, .. } => assert!(message.starts_with("Unable to create directory:\n")),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn rename_moves_the_entry() {
    let mut t = test_engine();
    std::fs::write(t.dir.path().join("old.txt"), b"x").unwrap();

    t.engine.handle_command(Command::Rename {
        location: Location::Local,
        from: t.dir.path().join("old.txt"),
        to: t.dir.path().join("new.txt"),
    });

    assert!(matches!(t.next_event().await, Event::LocationChanged { location: Location::Local }));
    assert!(t.dir.path().join("new.txt").exists());
    assert!(!t.dir.path().join("old.txt").exists());
}

#[tokio::test]
async fn renaming_onto_an_existing_local_file_is_refused_and_keeps_both() {
    let mut t = test_engine();
    std::fs::write(t.dir.path().join("small.dat"), b"x").unwrap();
    std::fs::write(t.dir.path().join("big.dat"), b"0123456789").unwrap();

    t.engine.handle_command(Command::Rename {
        location: Location::Local,
        from: t.dir.path().join("small.dat"),
        to: t.dir.path().join("big.dat"),
    });

    match t.next_event().await {
        Event::Notice { severity: Severity::Error, message } => assert_eq!(message, "\"big.dat\" already exists"),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(std::fs::read(t.dir.path().join("big.dat")).unwrap(), b"0123456789");
    assert!(t.dir.path().join("small.dat").exists());
}

#[tokio::test]
async fn renaming_onto_an_existing_remote_file_is_refused_and_keeps_both() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/small.dat", b"x", None).file("/big.dat", b"0123456789", None);

    t.engine.handle_command(Command::Rename {
        location: Location::Session(session),
        from: PathBuf::from("/small.dat"),
        to: PathBuf::from("/big.dat"),
    });

    match t.next_event().await {
        Event::Notice { severity: Severity::Error, message } => {
            assert_eq!(message, "Unable to rename:\n\"big.dat\" already exists");
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(remote.contents("/big.dat").unwrap(), b"0123456789");
    assert!(remote.exists("/small.dat"));
}

#[tokio::test]
async fn renaming_onto_an_existing_remote_folder_leaves_the_folder_in_place() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/a.txt", b"x", None).dir("/empty_dir").file("/empty_dir/inside.txt", b"y", None);

    t.engine.handle_command(Command::Rename {
        location: Location::Session(session),
        from: PathBuf::from("/a.txt"),
        to: PathBuf::from("/empty_dir"),
    });

    assert!(matches!(t.next_event().await, Event::Notice { severity: Severity::Error, .. }));
    assert!(remote.exists("/empty_dir/inside.txt"));
    assert!(!remote.exists("/empty_dir.bak"));
    assert!(remote.exists("/a.txt"));
}

#[tokio::test]
async fn renaming_to_the_same_name_changes_nothing_and_reports_nothing() {
    let mut t = test_engine();
    std::fs::write(t.dir.path().join("same.txt"), b"x").unwrap();

    t.engine.handle_command(Command::Rename {
        location: Location::Local,
        from: t.dir.path().join("same.txt"),
        to: t.dir.path().join("same.txt"),
    });

    assert!(matches!(t.next_event().await, Event::LocationChanged { location: Location::Local }));
    assert_eq!(std::fs::read(t.dir.path().join("same.txt")).unwrap(), b"x");
}

#[tokio::test]
async fn deleting_a_local_folder_emits_location_changed() {
    let mut t = test_engine();
    let target = t.dir.path().join("old");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("inside"), b"x").unwrap();

    t.engine.handle_command(Command::Delete { location: Location::Local, paths: vec![target.clone()] });

    assert!(matches!(t.next_event().await, Event::LocationChanged { location: Location::Local }));
    assert!(!target.exists());
}

#[tokio::test]
async fn deleting_stops_at_the_first_failure() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/a", b"", None).file("/c", b"", None);

    t.engine.handle_command(Command::Delete {
        location: Location::Session(session),
        paths: vec![PathBuf::from("/a"), PathBuf::from("/b"), PathBuf::from("/c")],
    });

    match t.next_event().await {
        Event::Notice { message, .. } => assert!(message.starts_with("Unable to delete:\n")),
        other => panic!("unexpected {other:?}"),
    }
    assert!(!remote.exists("/a"));
    assert!(remote.exists("/c"));
}

#[test]
fn an_absolute_start_directory_is_used_as_is() {
    assert_eq!(start_directory(Path::new("/home/user"), "/var/www"), PathBuf::from("/var/www"));
}

#[test]
fn a_relative_start_directory_is_under_home() {
    assert_eq!(start_directory(Path::new("/home/user"), "projects/app"), PathBuf::from("/home/user/projects/app"));
}

#[test]
fn a_tilde_start_directory_is_home() {
    assert_eq!(start_directory(Path::new("/home/user"), "~"), PathBuf::from("/home/user"));
}

#[test]
fn a_tilde_slash_start_directory_is_under_home() {
    assert_eq!(start_directory(Path::new("/home/user"), "~/projects"), PathBuf::from("/home/user/projects"));
}

#[test]
fn a_tilde_followed_by_a_name_is_a_relative_directory_not_another_users_home() {
    assert_eq!(start_directory(Path::new("/home/user"), "~backup"), PathBuf::from("/home/user/~backup"));
}

#[test]
fn each_failure_is_logged_at_the_line_that_reported_it() {
    let (events, _received) = tokio::sync::mpsc::unbounded_channel();
    let reporter = Reporter::new(events);

    let logs = crate::engine::testing::capture_logs(|| {
        send_failure(&reporter, "first".to_string(), &"cause");
        send_failure(&reporter, "second".to_string(), &"cause");
    });

    let places: Vec<&str> =
        logs.lines().filter(|line| line.contains("ERROR")).filter_map(|line| line.split("at=").nth(1)).collect();
    assert_eq!(places.len(), 2, "{logs}");
    assert_ne!(places[0], places[1], "{logs}");
    assert!(places[0].contains("fs/tests.rs"), "{logs}");
}

#[tokio::test]
async fn a_listing_is_followed_by_the_free_space_of_that_folder() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"", None).with_free_space(5_000);

    t.engine.handle_command(Command::List { location: Location::Session(session), path: Some(PathBuf::from("/data")) });

    assert!(matches!(t.next_event().await, Event::Listed { .. }));
    match t.next_event().await {
        Event::FreeSpace { location, path, bytes } => {
            assert_eq!(location, Location::Session(session));
            assert_eq!(path, PathBuf::from("/data"));
            assert_eq!(bytes, Some(5_000));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_server_that_cannot_tell_its_free_space_reports_none() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/data");

    t.engine.handle_command(Command::List { location: Location::Session(session), path: Some(PathBuf::from("/data")) });

    assert!(matches!(t.next_event().await, Event::Listed { .. }));
    assert!(matches!(t.next_event().await, Event::FreeSpace { bytes: None, .. }));
}

#[tokio::test]
async fn a_failing_free_space_query_reports_none_and_stays_quiet() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/data");
    remote.fail_free_space();

    t.engine.handle_command(Command::List { location: Location::Session(session), path: Some(PathBuf::from("/data")) });
    t.settle().await;
    let events = t.drain();

    assert!(events.iter().any(|event| matches!(event, Event::FreeSpace { bytes: None, .. })));
    assert!(!events.iter().any(|event| matches!(event, Event::Notice { .. })));
}

#[tokio::test]
async fn a_failed_listing_reports_no_free_space() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.with_free_space(5_000);

    t.engine.handle_command(Command::List { location: Location::Session(session), path: Some(PathBuf::from("/gone")) });
    t.settle().await;

    assert!(!t.drain().iter().any(|event| matches!(event, Event::FreeSpace { .. })));
}

#[tokio::test]
async fn listing_the_local_side_reports_the_free_space_of_the_disk() {
    let mut t = test_engine();
    let dir = t.dir.path().to_path_buf();

    t.engine.handle_command(Command::List { location: Location::Local, path: Some(dir.clone()) });

    assert!(matches!(t.next_event().await, Event::Listed { .. }));
    match t.next_event().await {
        Event::FreeSpace { location: Location::Local, path, bytes } => {
            assert_eq!(path, dir);
            assert!(bytes.unwrap() > 0);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn the_start_folder_listing_also_reports_free_space() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/home/user/work").with_free_space(9_000);
    t.engine.sessions.get_mut(&session).unwrap().entry.options.insert("remote_path".to_string(), "work".to_string());

    t.engine.open_start_directory(session);

    assert!(matches!(t.next_event().await, Event::Listed { .. }));
    assert!(matches!(t.next_event().await, Event::FreeSpace { bytes: Some(9_000), .. }));
}

#[tokio::test(start_paused = true)]
async fn a_free_space_query_that_never_answers_gives_up() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/data").hanging_free_space();

    t.engine.handle_command(Command::List { location: Location::Session(session), path: Some(PathBuf::from("/data")) });

    let listed = tokio::time::timeout(Duration::from_secs(60), t.events.recv()).await.unwrap().unwrap();
    let free = tokio::time::timeout(Duration::from_secs(60), t.events.recv()).await.unwrap().unwrap();
    assert!(matches!(listed, Event::Listed { .. }));
    assert!(matches!(free, Event::FreeSpace { bytes: None, .. }));
}

#[test]
fn a_rename_that_only_changes_the_case_is_not_a_collision() {
    assert!(differs_only_in_case(Path::new("/d/readme.md"), Path::new("/d/README.md")));
    assert!(!differs_only_in_case(Path::new("/d/readme.md"), Path::new("/d/other.md")));
}

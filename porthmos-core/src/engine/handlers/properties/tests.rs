use std::time::Duration;

use porthmos_lfs::from_os_path;
use porthmos_vfs::VfsPath;

use crate::{
    engine::{
        Command, Event, Location, RequestId,
        testing::{TestEngine, test_engine},
    },
    properties::{Digest, Measured},
    tasks::Scope,
};

const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn measured(bytes: u64, files: u64, folders: u64) -> Measured {
    Measured { bytes, files, folders, ..Measured::default() }
}

async fn next_measured(t: &mut TestEngine) -> (RequestId, Result<Measured, String>) {
    loop {
        if let Event::Measured { request_id, result } = t.next_event().await {
            return (request_id, result.map_err(|message| message.to_string()));
        }
    }
}

async fn next_checksummed(t: &mut TestEngine) -> (RequestId, VfsPath, Result<Digest, String>) {
    loop {
        if let Event::Checksummed { request_id, path, result } = t.next_event().await {
            return (request_id, path, result.map_err(|message| message.to_string()));
        }
    }
}

fn properties_events(events: Vec<Event>) -> Vec<Event> {
    events
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                Event::MeasureProgress { .. }
                    | Event::Measured { .. }
                    | Event::ChecksumProgress { .. }
                    | Event::Checksummed { .. }
            )
        })
        .collect()
}

fn paths(names: &[&str]) -> Vec<VfsPath> {
    names.iter().map(|name| VfsPath::new(name)).collect()
}

#[tokio::test]
async fn a_local_folder_is_measured_and_the_answer_carries_the_request_id() {
    let mut t = test_engine();
    let folder = t.dir.path().join("old");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    std::fs::write(folder.join("a"), b"xx").unwrap();
    std::fs::write(folder.join("sub").join("b"), b"yyy").unwrap();

    t.engine.handle_command(Command::Measure {
        request_id: 7,
        location: Location::Local,
        paths: vec![from_os_path(&folder)],
    });

    assert_eq!(next_measured(&mut t).await, (7, Ok(measured(5, 2, 1))));
}

#[tokio::test]
async fn a_remote_folder_is_measured_through_its_session() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"12", None).file("/data/deep/b", b"345", None).file("/other/c", b"6", None);

    t.engine.handle_command(Command::Measure {
        request_id: 1,
        location: Location::Session(session),
        paths: paths(&["/data"]),
    });

    assert_eq!(next_measured(&mut t).await, (1, Ok(measured(5, 2, 1))));
}

#[tokio::test]
async fn a_session_that_is_gone_answers_an_error_at_once() {
    let mut t = test_engine();

    t.engine.handle_command(Command::Measure {
        request_id: 3,
        location: Location::Session(99),
        paths: paths(&["/data"]),
    });
    t.engine.handle_command(Command::Checksum {
        request_id: 3,
        location: Location::Session(99),
        paths: paths(&["/a", "/b"]),
    });

    let (id, result) = next_measured(&mut t).await;
    assert_eq!(id, 3);
    assert!(result.unwrap_err().contains("session disconnected"));
    let first = next_checksummed(&mut t).await;
    let second = next_checksummed(&mut t).await;
    assert_eq!((first.1, second.1), (VfsPath::new("/a"), VfsPath::new("/b")));
    assert!(first.2.is_err() && second.2.is_err());
}

#[tokio::test]
async fn a_missing_folder_is_an_error_text_not_a_total() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");

    t.engine.handle_command(Command::Measure {
        request_id: 4,
        location: Location::Session(session),
        paths: paths(&["/missing"]),
    });

    let (id, result) = next_measured(&mut t).await;
    assert_eq!(id, 4);
    assert!(result.is_err());
}

#[tokio::test]
async fn a_connection_lost_mid_walk_gives_an_error_not_a_smaller_total() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"12", None).file("/data/deep/b", b"345", None);
    remote.fail_read_dir_with("/data/deep", porthmos_vfs::ErrorKind::Other);

    t.engine.handle_command(Command::Measure {
        request_id: 5,
        location: Location::Session(session),
        paths: paths(&["/data"]),
    });

    let (_, result) = next_measured(&mut t).await;
    assert!(result.is_err(), "{result:?}");
}

#[tokio::test]
async fn progress_arrives_before_the_final_result() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"12", None);

    t.engine.handle_command(Command::Measure {
        request_id: 6,
        location: Location::Session(session),
        paths: paths(&["/data"]),
    });

    match t.next_event().await {
        Event::MeasureProgress { request_id: 6, measured: seen } => assert_eq!(seen, measured(2, 1, 0)),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(next_measured(&mut t).await, (6, Ok(measured(2, 1, 0))));
}

#[tokio::test]
async fn a_checksum_sends_one_result_per_path_in_order() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/a", b"abc", None).file("/b", b"", None);

    t.engine.handle_command(Command::Checksum {
        request_id: 8,
        location: Location::Session(session),
        paths: paths(&["/a", "/b"]),
    });

    let first = next_checksummed(&mut t).await;
    let second = next_checksummed(&mut t).await;
    assert_eq!(first, (8, VfsPath::new("/a"), Ok(Digest { hex: ABC.to_string(), bytes: 3 })));
    assert_eq!(second.0, 8);
    assert_eq!(second.1, VfsPath::new("/b"));
    assert_eq!(second.2.unwrap().bytes, 0);
}

#[tokio::test]
async fn checksum_progress_reports_the_listed_size_as_the_total() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/a", b"abc", None);

    t.engine.handle_command(Command::Checksum {
        request_id: 9,
        location: Location::Session(session),
        paths: paths(&["/a"]),
    });

    match t.next_event().await {
        Event::ChecksumProgress { request_id: 9, path, bytes_done: 3, bytes_total: 3 } => {
            assert_eq!(path, VfsPath::new("/a"));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_folder_in_a_checksum_request_is_not_a_file() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/dir/x", b"1", None);

    t.engine.handle_command(Command::Checksum {
        request_id: 10,
        location: Location::Session(session),
        paths: paths(&["/dir"]),
    });

    let (_, _, result) = next_checksummed(&mut t).await;
    assert_eq!(result, Err("not a file".to_string()));
}

#[tokio::test]
async fn one_unreadable_file_does_not_stop_the_next() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/bad", b"abc", None).file("/good", b"abc", None);
    remote.fail_reads("/bad");

    t.engine.handle_command(Command::Checksum {
        request_id: 11,
        location: Location::Session(session),
        paths: paths(&["/bad", "/good"]),
    });

    let first = next_checksummed(&mut t).await;
    let second = next_checksummed(&mut t).await;
    assert!(first.2.is_err());
    assert_eq!(second.2.unwrap().hex, ABC);
}

#[tokio::test]
async fn a_new_request_id_cancels_the_jobs_of_the_old_one() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/stuck", b"abcdef", None).file("/other/a", b"1", None);
    remote.hanging_reads("/stuck", 0);
    let location = Location::Session(session);

    t.engine.handle_command(Command::Checksum { request_id: 1, location, paths: paths(&["/stuck"]) });
    tokio::time::sleep(Duration::from_millis(50)).await;
    t.engine.handle_command(Command::Measure { request_id: 2, location, paths: paths(&["/other"]) });

    assert_eq!(next_measured(&mut t).await, (2, Ok(measured(1, 1, 0))));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(t.engine.tasks.live_names().is_empty(), "{:?}", t.engine.tasks.live_names());
    assert!(properties_events(t.drain()).is_empty());
}

#[tokio::test]
async fn cancelling_stops_a_stalled_read_and_nothing_more_arrives() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/stuck", b"abcdef", None);
    remote.hanging_reads("/stuck", 0);

    t.engine.handle_command(Command::Checksum {
        request_id: 12,
        location: Location::Session(session),
        paths: paths(&["/stuck"]),
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    t.engine.handle_command(Command::CancelProperties { request_id: 12 });
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(t.engine.tasks.live_names().is_empty(), "{:?}", t.engine.tasks.live_names());
    assert!(properties_events(t.drain()).is_empty());
}

#[tokio::test]
async fn a_cancel_for_an_older_request_id_is_ignored() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"12", None);
    let location = Location::Session(session);

    t.engine.handle_command(Command::Measure { request_id: 2, location, paths: paths(&["/data"]) });
    t.engine.handle_command(Command::CancelProperties { request_id: 1 });

    assert_eq!(next_measured(&mut t).await, (2, Ok(measured(2, 1, 0))));
}

#[tokio::test]
async fn the_same_request_id_may_checksum_while_the_measure_runs() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"abc", None);
    let location = Location::Session(session);

    t.engine.handle_command(Command::Measure { request_id: 3, location, paths: paths(&["/data"]) });
    t.engine.handle_command(Command::Checksum { request_id: 3, location, paths: paths(&["/data/a"]) });

    assert_eq!(next_measured(&mut t).await, (3, Ok(measured(3, 1, 0))));
    assert_eq!(next_checksummed(&mut t).await.2.unwrap().hex, ABC);
}

#[tokio::test]
async fn cancelling_with_nothing_running_does_nothing() {
    let mut t = test_engine();

    t.engine.handle_command(Command::CancelProperties { request_id: 1 });

    assert!(t.drain().is_empty());
}

#[tokio::test]
async fn the_scopes_are_forgotten_after_a_cancel() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"12", None);

    t.engine.handle_command(Command::Measure {
        request_id: 1,
        location: Location::Session(session),
        paths: paths(&["/data"]),
    });
    t.engine.handle_command(Command::CancelProperties { request_id: 1 });

    assert!(!t.engine.tasks.take_cancelled(Scope::Measure));
    assert!(!t.engine.tasks.take_cancelled(Scope::Checksum));
}

#[tokio::test]
async fn disconnecting_the_session_stops_its_running_hash_and_nothing_more_arrives() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/stuck", b"abcdef", None);
    remote.hanging_reads("/stuck", 0);

    t.engine.handle_command(Command::Checksum {
        request_id: 20,
        location: Location::Session(session),
        paths: paths(&["/stuck"]),
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    t.engine.handle_command(Command::Disconnect { session });
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(t.engine.tasks.live_names().is_empty(), "{:?}", t.engine.tasks.live_names());
    assert!(properties_events(t.drain()).is_empty());
}

#[tokio::test]
async fn disconnecting_another_session_leaves_the_job_alone() {
    let mut t = test_engine();
    let (first, remote) = t.add_session("one");
    let (second, _other) = t.add_session("two");
    remote.file("/data/a", b"12", None);

    t.engine.handle_command(Command::Measure {
        request_id: 21,
        location: Location::Session(first),
        paths: paths(&["/data"]),
    });
    t.engine.handle_command(Command::Disconnect { session: second });

    assert_eq!(next_measured(&mut t).await, (21, Ok(measured(2, 1, 0))));
}

#[tokio::test]
async fn a_cancel_sent_before_the_job_first_runs_means_no_event_at_all() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    for index in 0..50 {
        remote.file(format!("/data/f{index}"), b"x", None);
    }

    t.engine.handle_command(Command::Measure {
        request_id: 22,
        location: Location::Session(session),
        paths: paths(&["/data"]),
    });
    t.engine.handle_command(Command::Checksum {
        request_id: 22,
        location: Location::Session(session),
        paths: paths(&["/data/f1"]),
    });
    t.engine.handle_command(Command::CancelProperties { request_id: 22 });
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(properties_events(t.drain()).is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn a_selected_link_on_the_real_local_filesystem_is_counted_not_followed() {
    let mut t = test_engine();
    let target = t.dir.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("big"), vec![0u8; 5_000]).unwrap();
    let link = t.dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    t.engine.handle_command(Command::Measure {
        request_id: 23,
        location: Location::Local,
        paths: vec![from_os_path(&link)],
    });

    let (_, result) = next_measured(&mut t).await;
    assert_eq!(result, Ok(Measured { symlinks: 1, ..Measured::default() }));
}

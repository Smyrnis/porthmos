use std::time::Duration;

use porthmos_lfs::from_os_path;
use porthmos_vfs::VfsPath;

use crate::{
    delete_prompt::count::{COUNT_LIMIT, ContentCount, Contents},
    engine::{Command, Event, Location, testing::test_engine},
};

fn counted(files: u64, folders: u64) -> Contents {
    Contents::Counted(ContentCount { files, folders, unreadable: 0, capped: false })
}

async fn next_count(t: &mut crate::engine::testing::TestEngine) -> (u64, Contents) {
    loop {
        if let Event::ContentsCounted { request_id, contents } = t.next_event().await {
            return (request_id, contents);
        }
    }
}

fn no_count_arrived(events: Vec<Event>) -> bool {
    !events.iter().any(|event| matches!(event, Event::ContentsCounted { .. }))
}

#[tokio::test]
async fn a_local_folder_is_counted_and_the_answer_carries_the_request_id() {
    let mut t = test_engine();
    let folder = t.dir.path().join("old");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    std::fs::write(folder.join("a"), b"x").unwrap();
    std::fs::write(folder.join("sub").join("b"), b"x").unwrap();

    t.engine.handle_command(Command::CountContents {
        request_id: 7,
        location: Location::Local,
        folders: vec![from_os_path(&folder)],
    });

    assert_eq!(next_count(&mut t).await, (7, counted(2, 1)));
}

#[tokio::test]
async fn a_remote_folder_is_counted_through_its_session() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"x", None).file("/data/deep/b", b"x", None).file("/other/c", b"x", None);

    t.engine.handle_command(Command::CountContents {
        request_id: 1,
        location: Location::Session(session),
        folders: vec![VfsPath::from("/data")],
    });

    assert_eq!(next_count(&mut t).await, (1, counted(2, 1)));
}

#[tokio::test]
async fn a_session_that_is_gone_answers_unavailable_at_once() {
    let mut t = test_engine();

    t.engine.handle_command(Command::CountContents {
        request_id: 3,
        location: Location::Session(99),
        folders: vec![VfsPath::from("/data")],
    });

    assert_eq!(next_count(&mut t).await, (3, Contents::Unavailable));
}

#[tokio::test]
async fn a_folder_that_cannot_be_listed_answers_unavailable() {
    let mut t = test_engine();
    let (session, _remote) = t.add_session("srv");

    t.engine.handle_command(Command::CountContents {
        request_id: 4,
        location: Location::Session(session),
        folders: vec![VfsPath::from("/missing")],
    });

    assert_eq!(next_count(&mut t).await, (4, Contents::Unavailable));
}

#[tokio::test]
async fn a_huge_folder_is_capped_at_the_limit() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    for index in 0..=COUNT_LIMIT {
        remote.file(format!("/big/f{index}"), b"", None);
    }

    t.engine.handle_command(Command::CountContents {
        request_id: 5,
        location: Location::Session(session),
        folders: vec![VfsPath::from("/big")],
    });

    match next_count(&mut t).await {
        (5, Contents::Counted(count)) => assert!(count.capped),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_cancelled_count_sends_no_answer() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/data/a", b"x", None);

    t.engine.handle_command(Command::CountContents {
        request_id: 6,
        location: Location::Session(session),
        folders: vec![VfsPath::from("/data")],
    });
    t.engine.handle_command(Command::CancelCount);
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(no_count_arrived(t.drain()));
}

#[tokio::test]
async fn a_new_count_replaces_the_one_still_running() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/first/a", b"x", None).file("/second/a", b"x", None).file("/second/b", b"x", None);
    let location = Location::Session(session);

    t.engine.handle_command(Command::CountContents { request_id: 1, location, folders: vec![VfsPath::from("/first")] });
    t.engine.handle_command(Command::CountContents {
        request_id: 2,
        location,
        folders: vec![VfsPath::from("/second")],
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let answers: Vec<(u64, Contents)> = t
        .drain()
        .into_iter()
        .filter_map(|event| match event {
            Event::ContentsCounted { request_id, contents } => Some((request_id, contents)),
            _ => None,
        })
        .collect();
    assert_eq!(answers, vec![(2, counted(2, 0))]);
}

#[tokio::test]
async fn cancelling_with_nothing_running_does_nothing() {
    let mut t = test_engine();

    t.engine.handle_command(Command::CancelCount);

    assert!(t.drain().is_empty());
}

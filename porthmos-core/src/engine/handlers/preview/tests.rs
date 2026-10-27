use std::{sync::Arc, time::Duration};

use porthmos_lfs::from_os_path;
use porthmos_vfs::VfsPath;

use crate::{
    engine::{
        Command, Event, Location,
        testing::{TestEngine, test_engine},
    },
    preview::{Preview, PreviewContent},
};

fn preview_events(events: &[Event]) -> Vec<(u64, String, Result<Arc<Preview>, String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::PreviewReady { request_id, name, result } => Some((*request_id, name.clone(), result.clone())),
            _ => None,
        })
        .collect()
}

fn ask(t: &mut TestEngine, request_id: u64, location: Location, path: &str) {
    t.engine.handle_command(Command::Preview { request_id, location, path: VfsPath::from(path) });
}

async fn next_preview(t: &mut TestEngine) -> (u64, String, Result<Arc<Preview>, String>) {
    loop {
        if let Event::PreviewReady { request_id, name, result } = t.next_event().await {
            return (request_id, name, result);
        }
    }
}

fn text(content: &str) -> PreviewContent {
    PreviewContent::Text(content.to_string())
}

#[tokio::test]
async fn a_local_file_is_previewed_with_its_name_and_the_request_id() {
    let mut t = test_engine();
    std::fs::write(t.dir.path().join("notes.txt"), "hello\nworld\n").unwrap();
    let path = t.dir.path().join("notes.txt");

    t.engine.handle_command(Command::Preview { request_id: 7, location: Location::Local, path: from_os_path(&path) });
    t.run_internal().await;

    let (id, name, result) = next_preview(&mut t).await;
    assert_eq!((id, name.as_str()), (7, "notes.txt"));
    let preview = result.unwrap();
    assert_eq!(preview.content, text("hello\nworld\n"));
    assert_eq!((preview.size, preview.truncated), (Some(12), false));
}

#[tokio::test]
async fn a_remote_file_is_previewed_through_its_session() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a.log", b"remote text", Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/a.log");
    t.run_internal().await;

    let (_, name, result) = next_preview(&mut t).await;
    assert_eq!(name, "a.log");
    assert_eq!(result.unwrap().content, text("remote text"));
}

#[tokio::test]
async fn a_big_file_is_cut_at_the_limit() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/big.txt", &vec![b'x'; crate::preview::PREVIEW_LIMIT + 5000], Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/big.txt");
    t.run_internal().await;

    let preview = next_preview(&mut t).await.2.unwrap();
    assert!(preview.truncated);
    assert_eq!(preview.shown, crate::preview::PREVIEW_LIMIT);
    assert_eq!(preview.size, Some((crate::preview::PREVIEW_LIMIT + 5000) as u64));
}

#[tokio::test]
async fn a_folder_missing_file_and_unreadable_file_each_say_what_went_wrong_with_the_name() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.dir("/srv/sub");
    remote.file("/srv/locked", b"x", Some(1));
    remote.fail_reads("/srv/locked");

    for (path, expected) in
        [("/srv/sub", "folder"), ("/srv/none", "No such file"), ("/srv/locked", "injected read failure")]
    {
        ask(&mut t, 1, Location::Session(session), path);
        t.run_internal().await;
        let (_, name, result) = next_preview(&mut t).await;
        let message = result.unwrap_err();
        assert!(message.starts_with(&format!("Couldn't preview {name}")), "{message}");
        assert!(message.contains(expected), "{message}");
    }
}

#[tokio::test]
async fn a_session_that_is_gone_is_an_immediate_error() {
    let mut t = test_engine();

    ask(&mut t, 3, Location::Session(99), "/srv/a.txt");

    let events = preview_events(&t.drain());
    assert_eq!(events.len(), 1);
    let (id, name, result) = &events[0];
    assert_eq!((*id, name.as_str()), (3, "a.txt"));
    assert!(result.as_ref().unwrap_err().contains("closed"), "{result:?}");
}

#[tokio::test]
async fn a_newer_request_replaces_an_older_one_whose_result_is_never_shown() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a.txt", b"first", Some(1)).file("/srv/b.txt", b"second", Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/a.txt");
    ask(&mut t, 2, Location::Session(session), "/srv/b.txt");
    t.run_internal().await;
    t.run_internal().await;

    let events = preview_events(&t.drain());
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, 2);
    assert_eq!(events[0].2.as_ref().unwrap().content, text("second"));
}

#[tokio::test]
async fn cancelling_drops_the_result() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a.txt", b"first", Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/a.txt");
    t.engine.handle_command(Command::CancelPreview);
    t.run_internal().await;

    assert!(preview_events(&t.drain()).is_empty());
}

#[tokio::test]
async fn a_result_after_the_answer_was_delivered_is_not_delivered_twice() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a.txt", b"first", Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/a.txt");
    t.run_internal().await;
    t.engine.finish_preview(1, "a.txt".to_string(), Err("late".to_string()));

    assert_eq!(preview_events(&t.drain()).len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_read_that_never_answers_gives_up_with_a_clear_message() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/hang.txt", b"x", Some(1)).hanging_stat("/srv/hang.txt");

    ask(&mut t, 4, Location::Session(session), "/srv/hang.txt");
    let done = tokio::time::timeout(Duration::from_secs(120), t.internal.recv()).await.unwrap().unwrap();
    t.engine.handle_internal(done);

    let (id, _, result) = preview_events(&t.drain()).remove(0);
    assert_eq!(id, 4);
    assert!(result.unwrap_err().contains("took too long"));
}

#[tokio::test]
async fn previewing_does_not_touch_the_transfer_queue_or_publish_notices() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a.txt", b"hello", Some(1));
    t.drain();

    ask(&mut t, 1, Location::Session(session), "/srv/a.txt");
    t.run_internal().await;

    let events = t.drain();
    assert!(events.iter().all(|event| matches!(event, Event::PreviewReady { .. })), "{events:?}");
    assert_eq!(t.engine.transfers.jobs().count(), 0);
}

#[tokio::test]
async fn a_hostile_name_is_carried_as_is_for_the_viewer_to_neutralise() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/we\u{1b}[31mird", b"hello", Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/we\u{1b}[31mird");
    t.run_internal().await;

    let (_, name, _) = next_preview(&mut t).await;
    assert_eq!(name, "we\u{1b}[31mird");
}

#[tokio::test]
async fn only_one_preview_task_runs_at_a_time_and_the_newest_request_wins() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/slow.txt", b"abcdef", Some(1)).hanging_reads("/srv/slow.txt", 3);
    remote.file("/srv/b.txt", b"second", Some(1)).file("/srv/c.txt", b"third", Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/slow.txt");
    tokio::time::sleep(Duration::from_millis(50)).await;
    ask(&mut t, 2, Location::Session(session), "/srv/b.txt");
    ask(&mut t, 3, Location::Session(session), "/srv/c.txt");

    assert_eq!(t.engine.tasks.live_names().iter().filter(|name| **name == "preview").count(), 1);
    t.run_internal().await;
    t.run_internal().await;

    let events = preview_events(&t.drain());
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, 3);
    assert_eq!(events[0].2.as_ref().unwrap().content, text("third"));
}

#[tokio::test]
async fn cancelling_stops_a_stalled_read_promptly_and_delivers_nothing() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/slow.txt", b"abcdef", Some(1)).hanging_reads("/srv/slow.txt", 3);
    ask(&mut t, 1, Location::Session(session), "/srv/slow.txt");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let started = std::time::Instant::now();

    t.engine.handle_command(Command::CancelPreview);
    t.run_internal().await;

    assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    assert!(preview_events(&t.drain()).is_empty());
    assert!(t.engine.tasks.live_names().iter().all(|name| *name != "preview"));
}

#[tokio::test]
async fn a_closed_connection_for_a_newer_request_drops_the_older_answer() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/a.txt", b"first", Some(1));

    ask(&mut t, 1, Location::Session(session), "/srv/a.txt");
    ask(&mut t, 2, Location::Session(99), "/srv/b.txt");
    t.run_internal().await;

    let events = preview_events(&t.drain());
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, 2);
    assert!(events[0].2.as_ref().unwrap_err().contains("closed"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_named_pipe_is_refused_without_blocking_the_engine() {
    let mut t = test_engine();
    let pipe = t.dir.path().join("pipe");
    let path = std::ffi::CString::new(pipe.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);

    t.engine.handle_command(Command::Preview { request_id: 1, location: Location::Local, path: from_os_path(&pipe) });
    t.run_internal().await;

    let (_, name, result) = next_preview(&mut t).await;
    assert_eq!(name, "pipe");
    assert!(result.unwrap_err().contains("not a regular file"));
}

#[tokio::test(start_paused = true)]
async fn a_read_that_stalls_midway_reports_the_stall_with_the_name() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/srv/slow.txt", b"abcdef", Some(1)).hanging_reads("/srv/slow.txt", 3);

    ask(&mut t, 5, Location::Session(session), "/srv/slow.txt");
    let done = tokio::time::timeout(Duration::from_secs(120), t.internal.recv()).await.unwrap().unwrap();
    t.engine.handle_internal(done);

    let (id, name, result) = preview_events(&t.drain()).remove(0);
    assert_eq!((id, name.as_str()), (5, "slow.txt"));
    let message = result.unwrap_err();
    assert!(message.starts_with("Couldn't preview slow.txt") && message.contains("took too long"), "{message}");
}

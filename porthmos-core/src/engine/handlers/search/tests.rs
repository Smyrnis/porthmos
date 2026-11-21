use porthmos_vfs::VfsPath;

use super::*;
use crate::engine::{Command, testing::test_engine};

fn found_names(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::SearchFound { entry, .. } => Some(entry.name.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn typing_a_pattern_streams_matching_results_back() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/target.log", b"", None).file("/home/user/other.txt", b"", None);

    t.engine.handle_command(Command::Search {
        request_id: 1,
        location: Location::Session(session),
        root: VfsPath::from("/home/user"),
        pattern: "*target*".to_string(),
    });

    let mut events = Vec::new();
    loop {
        let event = t.next_event().await;
        let done = matches!(event, Event::SearchDone { .. });
        events.push(event);
        if done {
            break;
        }
    }
    assert_eq!(found_names(&events), ["target.log"]);
}

#[tokio::test(start_paused = true)]
async fn rapid_pattern_changes_dispatch_only_one_search_for_the_final_pattern() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/aaa.log", b"", None).file("/home/user/bbb.log", b"", None);
    let location = Location::Session(session);

    for pattern in ["*b*", "*bb*", "*a*"] {
        t.engine.handle_command(Command::Search {
            request_id: 1,
            location,
            root: VfsPath::from("/home/user"),
            pattern: pattern.to_string(),
        });
    }
    tokio::time::sleep(SEARCH_DEBOUNCE * 2).await;

    let events = t.drain();
    let done = events.iter().filter(|event| matches!(event, Event::SearchDone { .. })).count();
    assert_eq!(done, 1, "expected exactly one dispatched search, got {events:?}");
    assert_eq!(found_names(&events), ["aaa.log"]);
}

#[tokio::test(start_paused = true)]
async fn cancelling_before_the_debounce_sends_nothing() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/aaa.log", b"", None);

    t.engine.handle_command(Command::Search {
        request_id: 1,
        location: Location::Session(session),
        root: VfsPath::from("/home/user"),
        pattern: "*a*".to_string(),
    });
    t.engine.handle_command(Command::CancelSearch);
    tokio::time::sleep(SEARCH_DEBOUNCE * 2).await;

    assert!(t.drain().is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_new_search_stops_the_previous_one_so_only_the_latest_streams_results() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/alpha.log", b"", None).file("/home/user/beta.log", b"", None);
    t.engine.search(1, Location::Session(session), VfsPath::from("/home/user"), "*alpha*".to_string());
    t.engine.search(2, Location::Session(session), VfsPath::from("/home/user"), "*beta*".to_string());

    let mut events = Vec::new();
    loop {
        let event = t.next_event().await;
        let done = matches!(event, Event::SearchDone { .. });
        events.push(event);
        if done {
            break;
        }
    }
    tokio::time::sleep(SEARCH_DEBOUNCE * 4).await;
    events.extend(t.drain());

    assert_eq!(found_names(&events), vec!["beta.log".to_string()]);
}

async fn search_done(t: &mut crate::engine::testing::TestEngine) -> (bool, usize) {
    loop {
        if let Event::SearchDone { truncated, unreadable, .. } = t.next_event().await {
            return (truncated, unreadable);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_search_that_could_not_read_a_folder_counts_it() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/a.log", b"", None);
    remote.dir("/home/user/locked").fail_read_dir("/home/user/locked");

    t.engine.handle_command(Command::Search {
        request_id: 1,
        location: Location::Session(session),
        root: VfsPath::from("/home/user"),
        pattern: "*.log".to_string(),
    });

    assert_eq!(search_done(&mut t).await, (false, 1));
}

#[tokio::test(start_paused = true)]
async fn a_search_that_read_everything_counts_no_unreadable_folders() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/a.log", b"", None);

    t.engine.handle_command(Command::Search {
        request_id: 1,
        location: Location::Session(session),
        root: VfsPath::from("/home/user"),
        pattern: "*.log".to_string(),
    });

    assert_eq!(search_done(&mut t).await, (false, 0));
}

#[tokio::test(start_paused = true)]
async fn every_event_of_a_search_carries_the_request_id_it_was_started_with() {
    let mut t = test_engine();
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/a.log", b"", None);

    t.engine.handle_command(Command::Search {
        request_id: 42,
        location: Location::Session(session),
        root: VfsPath::from("/home/user"),
        pattern: "*.log".to_string(),
    });
    let mut events = Vec::new();
    loop {
        let event = t.next_event().await;
        let done = matches!(event, Event::SearchDone { .. });
        events.push(event);
        if done {
            break;
        }
    }

    let ids: Vec<u64> = events
        .iter()
        .filter_map(|event| match event {
            Event::SearchFound { request_id, .. }
            | Event::SearchDone { request_id, .. }
            | Event::SearchFailed { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, [42, 42]);
}

#[tokio::test(start_paused = true)]
async fn the_configured_result_limit_caps_the_search_and_marks_it_truncated() {
    let mut t = test_engine();
    t.engine.search.max_results = 1;
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/aaa.log", b"", None).file("/home/user/bbb.log", b"", None);

    t.engine.handle_command(Command::Search {
        request_id: 1,
        location: Location::Session(session),
        root: VfsPath::from("/home/user"),
        pattern: "*.log".to_string(),
    });

    let mut events = Vec::new();
    loop {
        let event = t.next_event().await;
        let done = matches!(event, Event::SearchDone { .. });
        events.push(event);
        if done {
            break;
        }
    }
    assert_eq!(found_names(&events).len(), 1);
    assert!(matches!(events.last(), Some(Event::SearchDone { truncated: true, .. })), "{events:?}");
}

#[tokio::test(start_paused = true)]
async fn the_configured_depth_limit_keeps_the_search_out_of_deeper_folders() {
    let mut t = test_engine();
    t.engine.search.max_depth = 1;
    let (session, remote) = t.add_session("srv");
    remote.file("/home/user/top.log", b"", None).file("/home/user/a/near.log", b"", None).file(
        "/home/user/a/b/deep.log",
        b"",
        None,
    );

    t.engine.handle_command(Command::Search {
        request_id: 1,
        location: Location::Session(session),
        root: VfsPath::from("/home/user"),
        pattern: "*.log".to_string(),
    });

    let mut events = Vec::new();
    loop {
        let event = t.next_event().await;
        let done = matches!(event, Event::SearchDone { .. });
        events.push(event);
        if done {
            break;
        }
    }
    assert_eq!(found_names(&events).len(), 2);
    assert!(!found_names(&events).contains(&"deep.log".to_string()));
}

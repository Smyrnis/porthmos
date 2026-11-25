use std::time::Duration;

use iced::futures::{Stream, StreamExt};
use porthmos_core::ShellSize;
use porthmos_term::{TermSize, TerminalEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use super::*;
use crate::app::Message;
use crate::testing::shell_pipe;

async fn next_within<S: Stream + Unpin>(stream: &mut S, limit: Duration) -> Option<S::Item> {
    tokio::time::timeout(limit, stream.next()).await.ok().flatten()
}

#[tokio::test]
async fn into_transport_carries_bytes_both_ways() {
    let (stream, mut end) = shell_pipe();
    let mut transport = into_transport(stream);

    end.to_client.write_all(b"out").await.unwrap();
    let mut received = [0u8; 3];
    transport.reader.read_exact(&mut received).await.unwrap();
    transport.writer.write_all(b"in").await.unwrap();
    let mut sent = [0u8; 2];
    end.from_client.read_exact(&mut sent).await.unwrap();

    assert_eq!((&received, &sent), (b"out", b"in"));
}

#[tokio::test]
async fn resizing_the_transport_reports_the_new_shell_size() {
    let (stream, end) = shell_pipe();
    let transport = into_transport(stream);

    (transport.resize)(TermSize::clamped(100, 30));

    assert_eq!(*end.sizes.lock().unwrap(), vec![ShellSize { columns: 100, rows: 30 }]);
}

#[tokio::test]
async fn the_exit_status_travels_through_the_transport() {
    let (stream, end) = shell_pipe();
    let transport = into_transport(stream);

    end.exit.send(Some(4)).unwrap();

    assert_eq!(transport.exit.await.unwrap(), Some(4));
}

#[tokio::test]
async fn a_burst_of_redraws_becomes_one_message() {
    let (redraw, redraws) = watch::channel(0u64);
    let (_events, receiver) = mpsc::channel::<TerminalEvent>(8);
    let mut stream = Box::pin(tab_stream(9, redraws, receiver));

    for _ in 0..100 {
        redraw.send_modify(|count| *count += 1);
    }

    assert!(matches!(next_within(&mut stream, Duration::from_secs(1)).await, Some(Message::TerminalRedrawn)));
    assert!(next_within(&mut stream, Duration::from_millis(100)).await.is_none());
}

#[tokio::test]
async fn terminal_events_are_forwarded_with_their_tab_id() {
    let (_redraw, redraws) = watch::channel(0u64);
    let (events, receiver) = mpsc::channel(8);
    let mut stream = Box::pin(tab_stream(3, redraws, receiver));

    events.send(TerminalEvent::Title("vim".into())).await.unwrap();

    match next_within(&mut stream, Duration::from_secs(1)).await {
        Some(Message::TerminalEventReceived(3, TerminalEvent::Title(title))) => assert_eq!(title, "vim"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn the_drawer_shows_focus_only_while_it_can_take_keys() {
    let mut drawer = Drawer { open: true, focused: true, ..Drawer::default() };
    assert!(!drawer.shows_focus());

    drawer.wanted = vec![Wanted {
        id: 1,
        source: TerminalSource::Local,
        directory: VfsPath::new("/"),
        placement: Placement::NewTab,
    }];
    assert!(drawer.shows_focus());

    drawer.focused = false;
    assert!(!drawer.shows_focus());
}

#[cfg(unix)]
#[test]
fn dropped_paths_are_quoted_joined_and_followed_by_a_space() {
    let paths = [std::path::PathBuf::from("/tmp/my file.txt"), std::path::PathBuf::from("/tmp/it's $HOME")];

    let text = dropped_text(&paths).unwrap();

    assert_eq!(
        text,
        format!("{} {} ", porthmos_ssh::shell_quote("/tmp/my file.txt"), porthmos_ssh::shell_quote("/tmp/it's $HOME"))
    );
    assert!(!text.contains('\n') && !text.contains('\r'));
}

#[test]
fn a_dropped_path_with_control_characters_has_no_text() {
    for hostile in ["/tmp/a\nb", "/tmp/a\rb", "/tmp/a\u{1b}[31mb", "/tmp/a\u{7f}b"] {
        assert_eq!(dropped_text(&[std::path::PathBuf::from(hostile)]), None, "{hostile:?}");
    }
    assert_eq!(dropped_text(&[]), None);
}

#[test]
fn windows_quoting_refuses_every_character_a_shell_would_expand() {
    assert_eq!(windows_quoted("C:\\Users\\me\\a b.txt"), Some("\"C:\\Users\\me\\a b.txt\"".to_string()));
    for hostile in [
        "C:\\$(calc).txt",
        "C:\\`x`.txt",
        "C:\\%PATH%.txt",
        "C:\\a!b.txt",
        "C:\\a\"b.txt",
        "C:\\a&b.txt",
        "C:\\a;b.txt",
    ] {
        assert_eq!(windows_quoted(hostile), None, "{hostile:?}");
    }
}

fn bare_pane(id: u64) -> TerminalPane {
    TerminalPane {
        id,
        source: TerminalSource::Local,
        title: String::new(),
        session: None,
        frozen: None,
        events: Handover::new(mpsc::channel(1).1),
        exited: None,
        size: opening_size(),
        gesture: Gesture::Idle,
        search: search::SearchState::default(),
        directory: porthmos_term::DirectoryTracker::new(porthmos_term::DirectoryOwner::Local),
        hover: None,
    }
}

#[test]
fn a_new_pane_forms_a_single_leaf_tab_whose_id_is_the_panes_id() {
    let drawer = Drawer {
        panes: vec![bare_pane(4)],
        tabs: vec![TerminalTab { id: 4, layout: layout::Layout::Leaf(4), focus: 4 }],
        ..Drawer::default()
    };

    drawer.assert_consistent();
    assert_eq!(drawer.active_tab().unwrap().layout.leaves(), vec![4]);
    assert_eq!(drawer.active_pane().unwrap().id, 4);
    assert_eq!(drawer.tab_index_of(4), Some(0));
    assert_eq!(drawer.tab_index_of(9), None);
}

#[test]
#[should_panic]
fn the_consistency_check_rejects_a_pane_missing_from_every_layout() {
    let drawer = Drawer {
        panes: vec![bare_pane(4), bare_pane(5)],
        tabs: vec![TerminalTab { id: 4, layout: layout::Layout::Leaf(4), focus: 4 }],
        ..Drawer::default()
    };

    drawer.assert_consistent();
}

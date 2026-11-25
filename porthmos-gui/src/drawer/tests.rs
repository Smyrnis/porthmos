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

    drawer.wanted = Some(Wanted { id: 1, source: TerminalSource::Local, directory: VfsPath::new("/") });
    assert!(drawer.shows_focus());

    drawer.focused = false;
    assert!(!drawer.shows_focus());
}

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt, DuplexStream, ReadBuf},
    sync::{mpsc, oneshot},
    time::{sleep, timeout},
};

use super::*;
use crate::{TermSize, TerminalConfig, TerminalEvent, Transport};

const WAIT: Duration = Duration::from_secs(3);

struct Harness {
    session: TerminalSession,
    events: mpsc::Receiver<TerminalEvent>,
    to_terminal: DuplexStream,
    from_terminal: DuplexStream,
    resizes: Arc<Mutex<Vec<TermSize>>>,
    exit: oneshot::Sender<Option<u32>>,
}

fn start() -> Harness {
    let (to_terminal, terminal_reader) = tokio::io::duplex(65536);
    let (terminal_writer, from_terminal) = tokio::io::duplex(65536);
    let resizes = Arc::new(Mutex::new(Vec::new()));
    let recorder = resizes.clone();
    let (exit, exit_receiver) = oneshot::channel();
    let transport = Transport {
        reader: Box::new(terminal_reader),
        writer: Box::new(terminal_writer),
        resize: Box::new(move |size| recorder.lock().unwrap().push(size)),
        exit: exit_receiver,
    };
    let (session, events) = TerminalSession::spawn(transport, TermSize::clamped(80, 24), TerminalConfig::default());
    Harness { session, events, to_terminal, from_terminal, resizes, exit }
}

#[tokio::test]
async fn output_reaches_the_snapshot_and_bumps_the_redraw_counter() {
    let mut harness = start();
    let mut redraws = harness.session.redraws();

    harness.to_terminal.write_all(b"hello").await.unwrap();
    timeout(WAIT, redraws.wait_for(|count| *count > 0)).await.unwrap().unwrap();

    assert_eq!(harness.session.snapshot().row_text(0), "hello");
}

#[tokio::test]
async fn replies_to_queries_are_written_back_to_the_transport() {
    let mut harness = start();

    harness.to_terminal.write_all(b"\x1b[6n").await.unwrap();
    let mut reply = [0u8; 6];
    timeout(WAIT, harness.from_terminal.read_exact(&mut reply)).await.unwrap().unwrap();

    assert_eq!(&reply, b"\x1b[1;1R");
}

#[tokio::test]
async fn user_input_is_written_in_order() {
    let mut harness = start();

    harness.session.write(b"ab".to_vec());
    harness.session.write(b"cd".to_vec());
    let mut received = [0u8; 4];
    timeout(WAIT, harness.from_terminal.read_exact(&mut received)).await.unwrap().unwrap();

    assert_eq!(&received, b"abcd");
}

#[tokio::test]
async fn resize_updates_the_terminal_and_calls_the_resize_function() {
    let harness = start();

    harness.session.resize(TermSize { columns: 100, rows: 30 });
    harness.session.resize(TermSize { columns: 0, rows: 0 });

    assert_eq!(
        *harness.resizes.lock().unwrap(),
        [TermSize { columns: 100, rows: 30 }, TermSize { columns: 1, rows: 1 }]
    );
    assert_eq!(harness.session.snapshot().size, TermSize { columns: 1, rows: 1 });
}

#[tokio::test]
async fn exit_is_reported_once_with_the_status_after_eof() {
    let mut harness = start();

    drop(harness.to_terminal);
    harness.exit.send(Some(5)).unwrap();

    assert_eq!(timeout(WAIT, harness.events.recv()).await.unwrap(), Some(TerminalEvent::Exited(Some(5))));
    assert_eq!(timeout(WAIT, harness.events.recv()).await.unwrap(), None);
}

#[tokio::test]
async fn a_flood_of_bells_without_a_consumer_keeps_the_queue_bounded() {
    let mut harness = start();

    let flood = vec![0x07u8; 200_000];
    timeout(WAIT, harness.to_terminal.write_all(&flood)).await.unwrap().unwrap();
    drop(harness.to_terminal);
    harness.exit.send(Some(0)).unwrap();
    sleep(Duration::from_millis(300)).await;
    let mut received = Vec::new();
    while let Some(event) = timeout(WAIT, harness.events.recv()).await.unwrap() {
        received.push(event);
    }

    assert!(received.len() <= 65, "{} events were queued", received.len());
    assert_eq!(received.last(), Some(&TerminalEvent::Exited(Some(0))));
}

#[tokio::test]
async fn writing_after_the_remote_closed_neither_hangs_nor_panics() {
    let harness = start();
    drop(harness.to_terminal);
    drop(harness.from_terminal);
    sleep(Duration::from_millis(50)).await;

    timeout(WAIT, async {
        for _ in 0..10 {
            harness.session.write(vec![b'x'; 1000]);
            harness.session.resize(TermSize::clamped(90, 20));
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    assert_eq!(harness.session.snapshot().size, TermSize::clamped(90, 20));
}

#[tokio::test]
async fn a_stalled_remote_cannot_make_replies_pile_up() {
    let mut harness = start();
    let queries = "\x1b[c".repeat(8 << 20 >> 2);

    timeout(Duration::from_secs(20), harness.to_terminal.write_all(queries.as_bytes())).await.unwrap().unwrap();
    let mut delivered = 0usize;
    let mut buffer = vec![0u8; 65536];
    while let Ok(Ok(count)) = timeout(Duration::from_millis(500), harness.from_terminal.read(&mut buffer)).await {
        if count == 0 {
            break;
        }
        delivered += count;
    }

    assert!(delivered < 4 << 20, "{delivered} reply bytes were queued");
}

struct PanickingReader;

impl AsyncRead for PanickingReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>, _: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        panic!("the transport reader failed");
    }
}

#[tokio::test]
async fn a_panicking_pump_still_reports_the_exit() {
    let (_exit, exit_receiver) = oneshot::channel();
    let transport = Transport {
        reader: Box::new(PanickingReader),
        writer: Box::new(tokio::io::sink()),
        resize: Box::new(|_| {}),
        exit: exit_receiver,
    };
    let (_session, mut events) =
        TerminalSession::spawn(transport, TermSize::clamped(80, 24), TerminalConfig::default());

    assert_eq!(timeout(WAIT, events.recv()).await.unwrap(), Some(TerminalEvent::Exited(None)));
}

use std::sync::Arc;

use porthmos_vfs::{ShellSize, testing::FakeFs};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::engine::{
    Command, Event,
    testing::{TestEngine, test_engine},
};

const SIZE: ShellSize = ShellSize { columns: 80, rows: 24 };

async fn next_terminal_event(t: &mut TestEngine) -> Event {
    loop {
        let event = t.next_event().await;
        if matches!(event, Event::TerminalOpened { .. } | Event::TerminalOpenFailed { .. }) {
            return event;
        }
    }
}

#[tokio::test]
async fn opening_a_terminal_answers_with_the_stream() {
    let mut t = test_engine();
    let session = t.add_session_with("srv", Arc::new(FakeFs::new().with_shell()));

    t.engine.handle_command(Command::OpenTerminal { id: 5, session, size: SIZE });

    let Event::TerminalOpened { id, session: opened, stream } = next_terminal_event(&mut t).await else {
        panic!("expected the terminal to open");
    };
    assert_eq!((id, opened), (5, session));
    let mut stream = stream.take().expect("the first take gets the stream");
    stream.writer.write_all(b"hi").await.unwrap();
    let mut received = [0u8; 2];
    stream.reader.read_exact(&mut received).await.unwrap();
    assert_eq!(&received, b"hi");
}

#[tokio::test]
async fn opening_a_terminal_on_a_filesystem_without_shells_fails() {
    let mut t = test_engine();
    let (session, _fs) = t.add_session("srv");

    t.engine.handle_command(Command::OpenTerminal { id: 6, session, size: SIZE });

    let Event::TerminalOpenFailed { id, message } = next_terminal_event(&mut t).await else {
        panic!("expected a failure");
    };
    assert_eq!(id, 6);
    assert!(!message.to_string().is_empty());
}

#[tokio::test]
async fn opening_a_terminal_on_a_missing_session_fails() {
    let mut t = test_engine();

    t.engine.handle_command(Command::OpenTerminal { id: 7, session: 99, size: SIZE });

    assert!(matches!(next_terminal_event(&mut t).await, Event::TerminalOpenFailed { id: 7, .. }));
}

#[tokio::test]
async fn session_capabilities_report_embedded_shell_support() {
    let mut with = test_engine();
    let session = with.add_session_with("srv", Arc::new(FakeFs::new().with_shell()));
    let mut without = test_engine();
    let (plain, _fs) = without.add_session("srv");

    with.engine.emit_capabilities(session);
    without.engine.emit_capabilities(plain);

    let embedded = |events: Vec<Event>| {
        events.into_iter().find_map(|event| match event {
            Event::SessionCapabilities { embedded_shell, .. } => Some(embedded_shell),
            _ => None,
        })
    };
    assert_eq!(embedded(with.drain()), Some(true));
    assert_eq!(embedded(without.drain()), Some(false));
}

use std::time::Duration;

use porthmos_ssh::{
    open_shell_stream,
    testing::{Options, SshServer},
};
use porthmos_vfs::ShellSize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};

const TERM: &str = "xterm-256color";
const SIZE: ShellSize = ShellSize { columns: 80, rows: 24 };

async fn wait_for(mut condition: impl FnMut() -> bool) {
    for _ in 0..40 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the condition did not hold within two seconds");
}

async fn read_until(reader: &mut (impl AsyncReadExt + Unpin), needle: &str) {
    let mut received = Vec::new();
    let mut buffer = [0u8; 1024];
    timeout(Duration::from_secs(5), async {
        while !String::from_utf8_lossy(&received).contains(needle) {
            let count = reader.read(&mut buffer).await.unwrap();
            assert!(count > 0, "the stream ended before {needle:?} appeared");
            received.extend_from_slice(&buffer[..count]);
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{needle:?} never appeared in {:?}", String::from_utf8_lossy(&received)));
}

#[tokio::test]
async fn bytes_written_to_the_stream_reach_the_shell_and_output_comes_back() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let mut stream = open_shell_stream(&session, TERM, SIZE, &[]).await.unwrap();

    stream.writer.write_all(b"echo porthmos-ok\n").await.unwrap();

    read_until(&mut stream.reader, "porthmos-ok").await;
}

#[tokio::test]
async fn resizing_the_stream_sends_a_window_change() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let stream = open_shell_stream(&session, TERM, SIZE, &[]).await.unwrap();

    (stream.resize)(ShellSize { columns: 100, rows: 30 });

    wait_for(|| server.last_window_size() == Some((100, 30))).await;
}

#[tokio::test]
async fn the_exit_status_is_delivered_when_the_shell_ends() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let mut stream = open_shell_stream(&session, TERM, SIZE, &[]).await.unwrap();

    stream.writer.write_all(b"exit 3\n").await.unwrap();
    let mut rest = Vec::new();
    stream.reader.read_to_end(&mut rest).await.unwrap();

    let status = timeout(Duration::from_secs(5), stream.exit).await.expect("the exit status must arrive");
    assert_eq!(status.unwrap(), Some(3));
}

#[tokio::test]
async fn dropping_the_stream_ends_the_shell_on_the_server() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let stream = open_shell_stream(&session, TERM, SIZE, &[]).await.unwrap();
    wait_for(|| server.running_commands() == 1).await;

    drop(stream);

    wait_for(|| server.running_commands() == 0).await;
}

#[tokio::test]
async fn a_refused_shell_fails_the_open() {
    let server = SshServer::start(Options { refuse_shell: true, ..Options::default() }).await;
    let session = server.session().await;

    assert!(open_shell_stream(&session, TERM, SIZE, &[]).await.is_err());
}

#[tokio::test]
async fn a_burst_of_resizes_leaves_the_server_at_the_last_size() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let stream = open_shell_stream(&session, TERM, SIZE, &[]).await.unwrap();

    for step in 0..60u16 {
        (stream.resize)(ShellSize { columns: 81 + step, rows: 25 + step % 7 });
    }

    wait_for(|| server.last_window_size() == Some((140, 25 + 59 % 7))).await;
}

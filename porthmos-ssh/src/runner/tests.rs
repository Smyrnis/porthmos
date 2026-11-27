use std::{sync::Arc, time::Duration};

use porthmos_vfs::CommandRunner;
use tokio::{io::AsyncReadExt, time::timeout};

use super::SshRunner;
use crate::testing::SshServer;

async fn runner(server: &SshServer) -> SshRunner {
    SshRunner::new(Arc::new(server.session().await))
}

#[tokio::test]
async fn a_stream_hands_back_bytes_before_the_command_finishes() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let mut stream = runner.open_stream("sh -s", b"echo first\nsleep 30\necho never\n").await.unwrap();
    let mut buffer = [0u8; 6];
    timeout(Duration::from_secs(5), stream.output.read_exact(&mut buffer)).await.unwrap().unwrap();

    assert_eq!(&buffer, b"first\n");
    assert_eq!(server.running_commands(), 1);
}

#[tokio::test]
async fn a_stream_reports_status_and_stderr_after_eof() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let stream = runner.open_stream("sh -s", b"echo out; echo err >&2; exit 3\n").await.unwrap();
    let mut output = stream.output;
    let mut text = Vec::new();
    output.read_to_end(&mut text).await.unwrap();
    let end = timeout(Duration::from_secs(5), stream.ended).await.unwrap().unwrap();

    assert_eq!(text, b"out\n");
    assert_eq!(end.status, Some(3));
    assert_eq!(end.stderr, b"err\n");
}

#[tokio::test]
async fn the_reported_stderr_is_capped_at_four_kib() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let stream = runner.open_stream("sh -s", b"head -c 20000 /dev/zero | tr '\\0' e >&2\n").await.unwrap();
    let mut output = stream.output;
    let mut text = Vec::new();
    output.read_to_end(&mut text).await.unwrap();
    let end = timeout(Duration::from_secs(5), stream.ended).await.unwrap().unwrap();

    assert_eq!(end.stderr.len(), 4096);
}

#[tokio::test]
async fn ended_still_resolves_when_the_output_is_dropped_after_eof() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let stream = runner.open_stream("sh -s", b"echo hi\nexit 2\n").await.unwrap();
    let mut output = stream.output;
    let mut text = Vec::new();
    output.read_to_end(&mut text).await.unwrap();
    drop(output);
    let end = timeout(Duration::from_secs(5), stream.ended).await.unwrap().unwrap();

    assert_eq!(end.status, Some(2));
}

#[tokio::test]
async fn dropping_a_stream_mid_run_closes_the_channel() {
    let options = crate::testing::Options { max_sessions: Some(1), ..Default::default() };
    let server = SshServer::start(options).await;
    let runner = runner(&server).await;

    let mut stream = runner.open_stream("sh -s", b"echo first\nsleep 30\n").await.unwrap();
    let mut buffer = [0u8; 6];
    timeout(Duration::from_secs(5), stream.output.read_exact(&mut buffer)).await.unwrap().unwrap();
    assert!(runner.run("true").await.is_err());
    drop(stream);

    let mut reopened = false;
    for _ in 0..100 {
        if runner.run("true").await.is_ok() {
            reopened = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(reopened);
}

#[tokio::test]
async fn a_refused_exec_is_an_error() {
    let server = SshServer::start(crate::testing::Options { refuse_exec: true, ..Default::default() }).await;
    let runner = runner(&server).await;

    let refused = runner.open_stream("sh -s", b"true\n").await;

    assert!(refused.is_err() || refused.unwrap().ended.await.map_or(true, |end| end.status.is_none()));
}

async fn signals_after_drop(server: &SshServer) -> Vec<String> {
    for _ in 0..100 {
        let signals = server.received_signals();
        if !signals.is_empty() {
            return signals;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    server.received_signals()
}

#[tokio::test]
async fn dropping_a_stream_mid_run_asks_the_server_to_terminate_the_command() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let mut stream = runner.open_stream("sh -s", b"echo first\nsleep 30\n").await.unwrap();
    let mut buffer = [0u8; 6];
    timeout(Duration::from_secs(5), stream.output.read_exact(&mut buffer)).await.unwrap().unwrap();
    drop(stream);

    assert_eq!(signals_after_drop(&server).await, ["TERM"]);
}

#[tokio::test]
async fn an_empty_read_before_the_end_still_closes_the_channel_on_drop() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let mut stream = runner.open_stream("sh -s", b"printf 'first\\nsecond\\n'\nsleep 30\n").await.unwrap();
    let mut buffer = [0u8; 6];
    timeout(Duration::from_secs(5), stream.output.read_exact(&mut buffer)).await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut empty = [0u8; 0];
    assert_eq!(timeout(Duration::from_secs(5), stream.output.read(&mut empty)).await.unwrap().unwrap(), 0);
    drop(stream);

    assert_eq!(signals_after_drop(&server).await, ["TERM"]);
}

#[tokio::test]
async fn a_stream_read_to_its_end_sends_no_signal() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let stream = runner.open_stream("sh -s", b"echo hi\n").await.unwrap();
    let mut output = stream.output;
    let mut text = Vec::new();
    output.read_to_end(&mut text).await.unwrap();
    timeout(Duration::from_secs(5), stream.ended).await.unwrap().unwrap();
    drop(output);
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(server.received_signals().is_empty());
}

#[tokio::test]
async fn a_finished_run_with_input_sends_no_signal() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    runner.run_with_input("sh -s", b"echo hi\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(server.received_signals().is_empty());
}

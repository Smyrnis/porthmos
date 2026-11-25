#![cfg(unix)]

use std::{sync::Arc, time::Duration};

use porthmos_ssh::{SshRunner, testing::SshServer};
use porthmos_vfs::{CommandRunner, ErrorKind};
use tokio::time::timeout;

async fn runner(server: &SshServer) -> SshRunner {
    SshRunner::new(Arc::new(server.session().await))
}

#[tokio::test]
async fn a_command_returns_stdout_stderr_and_status() {
    let server = SshServer::start(Default::default()).await;

    let output = runner(&server).await.run("echo out; echo err >&2; exit 3").await.unwrap();

    assert_eq!(output.stdout, b"out\n");
    assert_eq!(output.stderr, b"err\n");
    assert_eq!(output.status, Some(3));
}

#[tokio::test]
async fn a_command_that_prints_nothing_succeeds_with_empty_output() {
    let server = SshServer::start(Default::default()).await;

    let output = runner(&server).await.run("true").await.unwrap();

    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(output.status, Some(0));
}

#[tokio::test]
async fn a_command_that_hangs_can_be_abandoned_by_the_caller_timeout() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let waited = timeout(Duration::from_millis(300), runner.run("sleep 30")).await;

    assert!(waited.is_err());
    let next = timeout(Duration::from_secs(5), runner.run("echo still-works")).await.unwrap().unwrap();
    assert_eq!(next.stdout, b"still-works\n");
}

#[tokio::test]
async fn many_commands_in_a_row_reuse_one_session() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    for index in 0..20 {
        let output = runner.run(&format!("echo {index}")).await.unwrap();
        assert_eq!(output.stdout, format!("{index}\n").into_bytes());
    }
}

#[tokio::test]
async fn output_far_beyond_the_cap_is_an_error_not_unbounded_memory() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;

    let error = timeout(Duration::from_secs(20), runner.run("head -c 20000000 /dev/zero"))
        .await
        .expect("the runner gives up instead of buffering everything")
        .expect_err("too much output is an error");

    assert_eq!(error.kind(), ErrorKind::Other);
    assert!(error.to_string().contains("too much output"), "{error}");
}

#[tokio::test]
async fn a_closed_session_is_an_error_not_a_hang() {
    let server = SshServer::start(Default::default()).await;
    let runner = runner(&server).await;
    runner.run("true").await.unwrap();

    server.disconnect_clients().await;

    let result = timeout(Duration::from_secs(5), runner.run("true")).await.expect("must not hang");
    assert!(result.is_err());
}

#[tokio::test]
async fn input_reaches_the_command_as_standard_input_and_not_its_arguments() {
    let server = SshServer::start(Default::default()).await;

    let output =
        runner(&server).await.run_with_input("sh -s", b"echo from-stdin\necho err >&2\nexit 4\n").await.unwrap();

    assert_eq!(output.stdout, b"from-stdin\n");
    assert_eq!(output.stderr, b"err\n");
    assert_eq!(output.status, Some(4));
}

#[tokio::test]
async fn a_large_input_is_delivered_whole() {
    let server = SshServer::start(Default::default()).await;
    let input = "x".repeat(200_000);

    let output = runner(&server).await.run_with_input("wc -c", input.as_bytes()).await.unwrap();

    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "200000");
}

#[tokio::test]
async fn endless_standard_error_is_cut_instead_of_buffered_without_bound() {
    let server = SshServer::start(Default::default()).await;

    let output =
        timeout(Duration::from_secs(30), runner(&server).await.run("head -c 20000000 /dev/zero >&2; echo done"))
            .await
            .expect("the command finishes")
            .unwrap();

    assert_eq!(output.stdout, b"done\n");
    assert!(output.stderr.len() <= 64 * 1024, "{} bytes of stderr kept", output.stderr.len());
    assert!(!output.stderr.is_empty());
}

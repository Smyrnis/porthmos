use std::time::Duration;

use porthmos_ssh::{
    ShellSize, open_shell,
    testing::{Options, PtyRequest, SshServer},
};
use tokio::{io::AsyncReadExt, time::timeout};

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

#[tokio::test]
async fn a_shell_runs_input_and_reports_the_exit_status() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;

    let mut channel = open_shell(&session, TERM, SIZE, &[]).await.unwrap();
    channel.input.send(b"echo porthmos-ok\nexit 3\n").await.unwrap();
    let mut received = Vec::new();
    channel.output.read_to_end(&mut received).await.unwrap();

    assert!(String::from_utf8_lossy(&received).contains("porthmos-ok"));
    assert_eq!(channel.finish().await, Some(3));
}

#[tokio::test]
async fn the_pty_request_carries_the_terminal_type_and_size() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;

    let _channel = open_shell(&session, TERM, SIZE, &[]).await.unwrap();

    wait_for(|| server.last_pty_request().is_some()).await;
    assert_eq!(server.last_pty_request(), Some(PtyRequest { term: TERM.into(), columns: 80, rows: 24 }));
}

#[tokio::test]
async fn resizing_sends_a_window_change() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let channel = open_shell(&session, TERM, SIZE, &[]).await.unwrap();

    channel.input.resize(ShellSize { columns: 100, rows: 30 }).await.unwrap();

    wait_for(|| server.last_window_size() == Some((100, 30))).await;
}

#[tokio::test]
async fn a_server_refusing_shells_fails_the_open() {
    let server = SshServer::start(Options { refuse_shell: true, ..Options::default() }).await;
    let session = server.session().await;

    assert!(open_shell(&session, TERM, SIZE, &[]).await.is_err());
}

#[tokio::test]
async fn input_after_the_shell_exits_fails_instead_of_hanging() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let mut channel = open_shell(&session, TERM, SIZE, &[]).await.unwrap();
    channel.input.send(b"exit\n").await.unwrap();
    let mut received = Vec::new();
    channel.output.read_to_end(&mut received).await.unwrap();

    let after = timeout(Duration::from_secs(2), async {
        (
            channel.input.send(b"x").await,
            channel.input.resize(ShellSize { columns: 10, rows: 10 }).await,
            channel.input.close().await,
        )
    })
    .await
    .expect("input after exit must not hang");

    assert!(after.0.is_err());
    assert!(after.1.is_err());
    assert!(after.2.is_err());
}

#[tokio::test]
async fn finishing_a_running_shell_does_not_wait_for_it_to_exit() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let channel = open_shell(&session, TERM, SIZE, &[]).await.unwrap();

    let status = timeout(Duration::from_secs(3), channel.finish()).await.expect("finish must not wait for the shell");

    assert_eq!(status, None);
}

#[tokio::test]
async fn environment_variables_are_offered_to_the_server() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;

    let _channel = open_shell(&session, TERM, SIZE, &[("LANG", "C.UTF-8")]).await.unwrap();

    wait_for(|| server.received_environment() == [("LANG".to_string(), "C.UTF-8".to_string())]).await;
}

#[tokio::test]
async fn dropping_a_running_shell_ends_it_on_the_server() {
    let server = SshServer::start(Options::default()).await;
    let session = server.session().await;
    let channel = open_shell(&session, TERM, SIZE, &[]).await.unwrap();
    wait_for(|| server.running_commands() == 1).await;

    drop(channel);

    wait_for(|| server.running_commands() == 0).await;
}

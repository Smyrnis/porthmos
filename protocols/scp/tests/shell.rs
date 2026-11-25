#![cfg(unix)]

use std::time::Duration;

use porthmos_scp::Scp;
use porthmos_ssh::testing::{self, Options, SshServer};
use porthmos_vfs::{Answer, FileSystem, Protocol, ShellSize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};

async fn connected(server: &SshServer) -> std::sync::Arc<dyn FileSystem> {
    Scp::default()
        .with_connect_options(server.connect_options())
        .connect(&server.target(), &mut testing::answers(vec![Some(Answer::Confirmed)]))
        .await
        .unwrap()
}

#[tokio::test]
async fn the_filesystem_reports_that_it_can_open_a_shell() {
    let server = SshServer::start(Options::default()).await;

    assert!(connected(&server).await.can_open_shell());
}

#[tokio::test]
async fn a_shell_opened_through_the_filesystem_echoes_input() {
    let server = SshServer::start(Options::default()).await;
    let fs = connected(&server).await;

    let mut shell = fs.open_shell("xterm-256color", ShellSize { columns: 80, rows: 24 }, &[]).await.unwrap();
    shell.writer.write_all(b"echo porthmos-ok\n").await.unwrap();

    let mut received = Vec::new();
    let mut buffer = [0u8; 1024];
    timeout(Duration::from_secs(5), async {
        while !String::from_utf8_lossy(&received).contains("porthmos-ok") {
            let count = shell.reader.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            received.extend_from_slice(&buffer[..count]);
        }
    })
    .await
    .expect("the shell output must arrive");
}
